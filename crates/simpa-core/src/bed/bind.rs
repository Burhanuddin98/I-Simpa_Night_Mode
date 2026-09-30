//! M8b: every file of a run bound to its record before the bed reads a number from it
//! (`docs/v1.1-backlog.md` rows 41 and 42, `docs/investigations/2026-09-30-m8b-tamper/FIXES.md`).
//!
//! The bed reads a run only when every file under its folder is, byte for byte, the file its
//! record names, with none gone and none added ([`Records`]). What binds a run is a record the
//! folder being judged does not hold:
//! - **what this process made** (`run::made_record`): a run the bed has just made, held to its
//!   `project.simpa` as the bed wrote it, and to its run folder as the run manager hashed it
//!   (`RunManifest::outputs`) and wrote its `run.json` when the run ended, all kept in memory;
//! - **a seal** ([`Seal`]): for a run of an earlier bed, one file listing every file under each
//!   run's folder in the bed (`runs/<cell>/s<seed>/`: its `project.simpa`, and the run folder with
//!   its `run.json`), with its size and sha256. Its `why` says what justified sealing the files as
//!   they then were. It is evidence only from outside the bed folder ([`seal_outside`]), and the
//!   gate takes it only as a committed file of the repository. A seal holds its whole bed: a run
//!   it does not name is refused.
//!
//! `run.json`'s `outputs` are held too when a run records them, but they bind nothing on their
//! own: `run.json` sits in the folder being judged, and an edit of that folder can forge it with
//! the files it names (`docs/investigations/2026-09-30-m8b-tamper/VERIFY-adversarial-1.md`,
//! finding 1). A run with no binding record is refused, [`RUN_UNBOUND`], never judged; a file
//! that is not its record's, [`FILES_CHANGED`]. What the bound files hold is then matched with
//! the plan (`run::check_planned`), and no two seeds of a cell may share a solver output file
//! (`run::into_reads`, [`SEEDS_IDENTICAL`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::run::manager::SOLVE_DIR;
use crate::run::manifest::{FILE_NAME as RUN_JSON, FileRef, OutputRef, hash_tree, sha256_bytes};

/// A run that no record binds: not made by this process, and named by no seal.
pub const RUN_UNBOUND: &str = "bed_run_unbound";
/// A file in a run's folder that is not its record's: changed, gone, or added.
pub const FILES_CHANGED: &str = "bed_run_files_changed";
/// Two seeds of one cell that share a solver output file byte for byte.
pub const SEEDS_IDENTICAL: &str = "bed_seed_outputs_identical";

/// The layout of [`Seal`].
pub const SEAL_VERSION: u32 = 1;

/// A committed record of an earlier bed's files ([module docs](self)).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seal {
    /// [`SEAL_VERSION`].
    pub seal_version: u32,
    /// The bed's folder name, `<UTC stamp>`: `--from` must name a folder of this name.
    pub bed: String,
    /// Where the files were read when they were sealed, and when (UTC).
    pub sealed_from: String,
    pub sealed_utc: String,
    /// The program that wrote the seal.
    pub tool: String,
    /// Why the files could be sealed as they were.
    pub why: String,
    /// Facts the seal's maker found about the bed, as it wrote them (the newest file under
    /// `runs/` against `report.json`'s time, say). Reported, never checked.
    #[serde(default)]
    pub provenance: BTreeMap<String, String>,
    /// The bed's own `report.json` and `summary.json` as they were when sealed.
    pub report_json_sha256: String,
    pub summary_json_sha256: String,
    /// Files and bytes over every run.
    pub files: u64,
    pub bytes: u64,
    /// By the run's folder under the bed, `runs/<cell>/s<seed>`.
    pub runs: BTreeMap<String, SealedRun>,
}

/// One run's folder under a sealed bed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealedRun {
    /// The run folder the run manager made there (`<stamp>-<solver>`).
    pub run_folder: String,
    /// Every file under the run's folder: `[path, size, sha256]`, the path relative to that
    /// folder with `/` between its parts.
    pub files: Vec<SealedFile>,
}

/// `[path, size, sha256]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SealedFile(pub String, pub u64, pub String);

/// A run's folder under the bed as a seal names it: `runs/<cell>/s<seed>`.
pub fn seal_key(key: &super::run::RunKey) -> String {
    key.dir()
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Seal {
    /// Reads and checks a seal file; returns it with the file's own sha256, its line ends as git
    /// stores them (`\n`), so that a checkout's line-end conversion does not change it.
    pub fn load(path: &Path) -> Result<(Seal, String), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let seal: Seal = serde_json::from_str(&text)
            .map_err(|e| format!("{}: not a seal: {e}", path.display()))?;
        seal.check()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok((seal, sha256_bytes(text.replace("\r\n", "\n").as_bytes())))
    }

    /// The seal's own consistency: its version, a sha256 and a relative path to every file, one
    /// entry per path, a `run.json` in each run folder, and totals that add up.
    pub fn check(&self) -> Result<(), String> {
        if self.seal_version != SEAL_VERSION {
            return Err(format!(
                "seal version {}, this build reads {SEAL_VERSION}",
                self.seal_version
            ));
        }
        let (mut files, mut bytes) = (0u64, 0u64);
        for (key, run) in &self.runs {
            let mut seen = BTreeSet::new();
            for SealedFile(path, size, sha) in &run.files {
                let bad_path = path.is_empty()
                    || path.starts_with('/')
                    || path.contains('\\')
                    || path
                        .split('/')
                        .any(|c| c.is_empty() || c == "." || c == "..");
                if bad_path || !is_sha256(sha) || !seen.insert(path.as_str()) {
                    return Err(format!(
                        "{key}: the entry {path:?} {size} {sha:?} is not valid"
                    ));
                }
                files += 1;
                bytes += size;
            }
            if !seen.contains(format!("{}/{RUN_JSON}", run.run_folder).as_str()) {
                return Err(format!("{key}: no {RUN_JSON} in {}", run.run_folder));
            }
        }
        if (files, bytes) != (self.files, self.bytes) {
            return Err(format!(
                "it lists {files} files and {bytes} bytes, its totals say {} and {}",
                self.files, self.bytes
            ));
        }
        Ok(())
    }

    /// Refuses a bed folder `from` that is not the one sealed.
    pub fn for_bed(&self, from: &Path) -> Result<(), String> {
        let name = from.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == self.bed {
            Ok(())
        } else {
            Err(format!(
                "the seal is of the bed {}, not of {}",
                self.bed,
                from.display()
            ))
        }
    }

    /// The run filed under `key`, when the seal has it.
    pub fn run(&self, key: &super::run::RunKey) -> Option<&SealedRun> {
        self.runs.get(&seal_key(key))
    }

    /// A seal of the bed folder named `bed` holding `runs` (by `runs/<cell>/s<seed>`), its
    /// totals added up. The caller says where, when, by what and why, and names the bed's
    /// `report.json` and `summary.json`.
    pub fn of_runs(bed: &str, runs: BTreeMap<String, SealedRun>) -> Seal {
        let files = runs.values().map(|r| r.files.len() as u64).sum();
        let bytes = runs.values().flat_map(|r| &r.files).map(|f| f.1).sum();
        Seal {
            seal_version: SEAL_VERSION,
            bed: bed.to_string(),
            sealed_from: String::new(),
            sealed_utc: String::new(),
            tool: String::new(),
            why: String::new(),
            provenance: BTreeMap::new(),
            report_json_sha256: String::new(),
            summary_json_sha256: String::new(),
            files,
            bytes,
            runs,
        }
    }

    /// The seal as written: pretty JSON with `\n` line ends, and a final one.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a seal serialises") + "\n"
    }
}

/// Refuses a seal file `seal` that lies inside the bed folder `from` it would hold: an edit of
/// the bed folder can edit such a seal with the files it names, so it binds nothing. Both are
/// compared as the file system resolves them.
pub fn seal_outside(from: &Path, seal: &Path) -> Result<(), String> {
    let resolve = |p: &Path| std::fs::canonicalize(p).map_err(|e| format!("{}: {e}", p.display()));
    let (bed, file) = (resolve(from)?, resolve(seal)?);
    if file.starts_with(&bed) {
        Err(format!(
            "the seal {} is inside the bed folder {} it would hold: an edit of the bed can edit \
             it too. Give a copy committed outside it (beds/<bed>-<stamp>/outputs-seal.json)",
            seal.display(),
            from.display()
        ))
    } else {
        Ok(())
    }
}

/// What a run's binding found: which records hold it, and its solver's outputs.
#[derive(Clone, Debug, PartialEq)]
pub struct Bound {
    /// `run.json`, `seal`, or both.
    pub by: Vec<&'static str>,
    /// The solver's outputs: every file under `solve/` that is not one of `run.json`'s inputs,
    /// as `(path under solve/, sha256)`, sorted by path.
    pub outputs: Vec<(String, String)>,
    /// The sha256 of `outputs`, one `path\tsha256\n` line per file.
    pub outputs_sha256: String,
}

/// `record` against `disk` (path to `(size, sha256)`): every file of the record there with its
/// size and sha256, and no other.
fn differences(
    what: &str,
    record: &BTreeMap<&str, (u64, &str)>,
    disk: &BTreeMap<&str, (u64, &str)>,
) -> Vec<String> {
    let mut out = Vec::new();
    for (path, (size, sha)) in record {
        match disk.get(path) {
            None => out.push(format!("{path} is gone ({what} has it)")),
            Some((s, h)) if s != size || h != sha => out.push(format!(
                "{path} has {s} bytes of sha256 {}, {what} {size} bytes of sha256 {}",
                &h[..h.len().min(12)],
                &sha[..sha.len().min(12)]
            )),
            Some(_) => {}
        }
    }
    for path in disk.keys().filter(|p| !record.contains_key(*p)) {
        out.push(format!("{path} is not in {what}"));
    }
    out
}

/// What a run's files are held to ([`bind`], [module docs](self)).
#[derive(Clone, Copy, Debug, Default)]
pub struct Records<'a> {
    /// A run this process has just made: every file under its folder in the bed as the bed wrote
    /// it and the run manager hashed and wrote it when the run ended, from memory
    /// (`run::made_record`), in a seal's form.
    pub made: Option<&'a SealedRun>,
    /// A run of an earlier bed: its seal's entry.
    pub sealed: Option<&'a SealedRun>,
    /// Its `run.json`'s `outputs`, as read from the run folder: held when there, never binding on
    /// their own.
    pub run_json: Option<&'a [OutputRef]>,
}

/// Binds the run in `folder`, the run folder under the run's folder `dir` in the bed
/// (`runs/<cell>/s<seed>`), to its records: `made` and `sealed` each for every file under `dir`;
/// `run_json` for the files in `folder` but `run.json`. `inputs` are its `run.json`'s inputs
/// (paths under `solve/`), which are not the solver's outputs. Refused [`RUN_UNBOUND`] when
/// neither `made` nor `sealed` is there, whatever `run.json` records; [`FILES_CHANGED`] when a
/// file is not a record's.
pub fn bind(
    dir: &Path,
    folder: &Path,
    records: Records<'_>,
    inputs: &[FileRef],
) -> Result<Bound, String> {
    let Records {
        made,
        sealed,
        run_json: outputs,
    } = records;
    let name = folder
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{}: no run folder name", folder.display()))?;
    if made.is_none() && sealed.is_none() {
        return Err(format!(
            "{RUN_UNBOUND}: {}: this process did not make it and no seal names it, so its files \
             are bound to nothing outside the folder being judged (its {RUN_JSON}'s output \
             hashes, {}, are in that folder and bind nothing on their own), and it is not judged",
            folder.display(),
            if outputs.is_some() {
                "recorded"
            } else {
                "not recorded: a run made before M8b"
            }
        ));
    }
    let tree = hash_tree(dir, &[]).map_err(|e| format!("{FILES_CHANGED}: {e}"))?;
    let disk: BTreeMap<&str, (u64, &str)> = tree
        .iter()
        .map(|f| (f.path.as_str(), (f.size, f.sha256.as_str())))
        .collect();
    let in_folder = format!("{name}/");
    let mut by = Vec::new();
    let mut problems = Vec::new();
    if let Some(outputs) = outputs {
        by.push("run.json");
        let record: BTreeMap<&str, (u64, &str)> = outputs
            .iter()
            .map(|f| (f.path.as_str(), (f.size, f.sha256.as_str())))
            .collect();
        let here: BTreeMap<&str, (u64, &str)> = disk
            .iter()
            .filter_map(|(p, v)| {
                p.strip_prefix(&in_folder)
                    .filter(|r| *r != RUN_JSON)
                    .map(|r| (r, *v))
            })
            .collect();
        problems.extend(
            differences("run.json's outputs", &record, &here)
                .into_iter()
                .map(|p| format!("{name}/{p}")),
        );
    }
    for (s, what, label) in [
        (made, "what this process made", "this process"),
        (sealed, "the seal", "seal"),
    ] {
        let Some(s) = s else { continue };
        by.push(label);
        if s.run_folder != name {
            problems.push(format!(
                "{what} names the run folder {}, the bed found {name}",
                s.run_folder
            ));
        }
        let record: BTreeMap<&str, (u64, &str)> = s
            .files
            .iter()
            .map(|SealedFile(p, size, sha)| (p.as_str(), (*size, sha.as_str())))
            .collect();
        problems.extend(differences(what, &record, &disk));
    }
    if !problems.is_empty() {
        let n = problems.len();
        let mut shown: Vec<String> = problems.into_iter().take(6).collect();
        if n > shown.len() {
            shown.push(format!("and {} more", n - shown.len()));
        }
        let records = format!("the {} record", by.join(" and "));
        return Err(format!(
            "{FILES_CHANGED}: {}: {n} difference(s) from what {records}: {}",
            dir.display(),
            shown.join("; ")
        ));
    }
    let solve = format!("{in_folder}{SOLVE_DIR}/");
    let outputs: Vec<(String, String)> = disk
        .iter()
        .filter_map(|(p, (_, sha))| {
            p.strip_prefix(&solve)
                .filter(|r| !inputs.iter().any(|i| i.path == *r))
                .map(|r| (r.to_string(), sha.to_string()))
        })
        .collect();
    let digest: String = outputs.iter().map(|(p, h)| format!("{p}\t{h}\n")).collect();
    Ok(Bound {
        by,
        outputs_sha256: sha256_bytes(digest.as_bytes()),
        outputs,
    })
}

/// `config.xml`'s text with the value of its first `workingdirectory` attribute left out: the one
/// thing in it that depends on where the run was made, not on what it runs.
pub fn without_workdir(text: &str) -> String {
    const ATTR: &str = "workingdirectory=\"";
    match text.find(ATTR) {
        Some(i) => {
            let start = i + ATTR.len();
            match text[start..].find('"') {
                Some(len) => format!("{}{}", &text[..start], &text[start + len..]),
                None => text.to_string(),
            }
        }
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A fresh run's folder `runs/c/s1` in the temporary folder, laid out as the bed leaves one:
    /// `project.simpa`, and the run folder `R` with its `run.json`, a log, `mesh/` and `solve/`
    /// (inputs `config.xml` and `mesh.cbin`, outputs a `.recp` and a `.gabe`). Returns it with
    /// the run folder, and the inputs `run.json` would record.
    fn run_dir(label: &str) -> (PathBuf, PathBuf, Vec<FileRef>) {
        static N: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "simpa-bed-bind-{label}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let dir = root.join("runs").join("c").join("s1");
        let folder = dir.join("R");
        for (path, text) in [
            ("project.simpa", "{}"),
            ("R/run.json", "{}"),
            ("R/solver.stdout.txt", "#99.99"),
            ("R/mesh/mesh.json", "{}"),
            ("R/solve/config.xml", "<configuration/>"),
            ("R/solve/mesh.cbin", "scene"),
            ("R/solve/Punctual receivers/R000/Sound level.recp", "seed 3"),
            ("R/solve/SPPS particle statistics.gabe", "stats 3"),
        ] {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, text).unwrap();
        }
        let inputs = ["config.xml", "mesh.cbin"]
            .iter()
            .map(|p| FileRef {
                path: p.to_string(),
                sha256: crate::run::manifest::sha256_file(&folder.join("solve").join(p)).unwrap(),
            })
            .collect();
        (dir, folder, inputs)
    }

    /// Removes the test's own temporary folder, once it has passed.
    fn done(dir: &Path) {
        let root = dir.parent().unwrap().parent().unwrap().parent().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    fn sealed(dir: &Path) -> SealedRun {
        SealedRun {
            run_folder: "R".into(),
            files: hash_tree(dir, &[])
                .unwrap()
                .into_iter()
                .map(|f| SealedFile(f.path, f.size, f.sha256))
                .collect(),
        }
    }

    /// `records` with `made`, `sealed` and `run_json` as given.
    fn records<'a>(
        made: Option<&'a SealedRun>,
        sealed: Option<&'a SealedRun>,
        run_json: Option<&'a [OutputRef]>,
    ) -> Records<'a> {
        Records {
            made,
            sealed,
            run_json,
        }
    }

    /// M8b round 2, `VERIFY-adversarial-1.md` finding 1: with no seal, a run was bound to its own
    /// `run.json`'s output hashes, and `run.json` sits in the folder being judged. U1 and U2 wrote
    /// a forged `run.json` whose `outputs` are the files the folder holds, and the bed passed on
    /// copied seeds. `run.json` alone binds nothing: a run neither made by this process nor named
    /// by a seal is refused `bed_run_unbound`, even when every file is what its `run.json` says.
    #[test]
    fn a_run_bound_only_by_its_own_run_json_is_refused() {
        let (dir, folder, inputs) = run_dir("runjson-alone");
        // The forged record: run.json's outputs written for the files the folder holds.
        let outputs = hash_tree(&folder, &[RUN_JSON]).unwrap();
        assert_eq!(outputs.len(), 6, "{outputs:?}");
        let e = bind(&dir, &folder, records(None, None, Some(&outputs)), &inputs).unwrap_err();
        assert!(e.starts_with(RUN_UNBOUND), "{e}");
        assert!(
            e.contains("this process did not make it and no seal names it"),
            "{e}"
        );
        assert!(
            e.contains("output hashes, recorded, are in that folder"),
            "{e}"
        );
        // With a record from outside the folder beside it, it binds.
        let made = sealed(&dir);
        let b = bind(
            &dir,
            &folder,
            records(Some(&made), None, Some(&outputs)),
            &inputs,
        )
        .unwrap();
        assert_eq!(b.by, ["run.json", "this process"]);
        done(&dir);
    }

    /// M8b round 2: a run this process made, held to what it made (`run::made_record`, from
    /// memory) and to its `run.json`'s outputs. As the run left them it is read, and its solver
    /// outputs are the files under `solve/` that are not inputs; an output changed, an input of
    /// the same size changed, a file added, `run.json` forged or `project.simpa` changed is refused
    /// `bed_run_files_changed`, naming the file. `run.json`'s outputs are held too: a record of
    /// what was made that the folder matches, but `run.json` does not, is refused.
    #[test]
    fn a_run_made_by_this_process_is_refused_when_any_file_is_not_what_it_made() {
        let (dir, folder, inputs) = run_dir("made");
        let outputs = hash_tree(&folder, &[RUN_JSON]).unwrap();
        let made = sealed(&dir);
        let bind_made = || {
            bind(
                &dir,
                &folder,
                records(Some(&made), None, Some(&outputs)),
                &inputs,
            )
        };
        let b = bind_made().unwrap();
        assert_eq!(b.by, ["run.json", "this process"]);
        let names: Vec<&str> = b.outputs.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            names,
            [
                "Punctual receivers/R000/Sound level.recp",
                "SPPS particle statistics.gabe"
            ]
        );
        // An output file overwritten with another seed's: refused, named.
        let recp = folder.join("solve/Punctual receivers/R000/Sound level.recp");
        std::fs::write(&recp, "seed 4").unwrap();
        let e = bind_made().unwrap_err();
        assert!(e.starts_with(FILES_CHANGED), "{e}");
        assert!(
            e.contains("R/solve/Punctual receivers/R000/Sound level.recp has 6 bytes of sha256"),
            "{e}"
        );
        assert!(e.contains("what this process made"), "{e}");
        // ... also when run.json is forged to match it (U1): what was made is not in the folder.
        let forged = hash_tree(&folder, &[RUN_JSON]).unwrap();
        let forged_json = "{\"outputs\": \"forged\"}";
        std::fs::write(folder.join(RUN_JSON), forged_json).unwrap();
        let e = bind(
            &dir,
            &folder,
            records(Some(&made), None, Some(&forged)),
            &inputs,
        )
        .unwrap_err();
        let want = format!("R/run.json has {} bytes", forged_json.len());
        assert!(e.contains(&want), "{e}");
        assert!(e.contains("Sound level.recp has 6 bytes"), "{e}");
        std::fs::write(folder.join(RUN_JSON), "{}").unwrap();
        std::fs::write(&recp, "seed 3").unwrap();
        assert!(bind_made().is_ok());
        // An input of the same size and another content: refused.
        std::fs::write(folder.join("solve/config.xml"), "<configuratiom/>").unwrap();
        let e = bind_made().unwrap_err();
        assert!(e.contains("R/solve/config.xml has 16 bytes"), "{e}");
        std::fs::write(folder.join("solve/config.xml"), "<configuration/>").unwrap();
        // project.simpa, which the bed wrote beside the run folder: refused.
        std::fs::write(dir.join("project.simpa"), "{ }").unwrap();
        let e = bind_made().unwrap_err();
        assert!(e.contains("project.simpa has 3 bytes"), "{e}");
        std::fs::write(dir.join("project.simpa"), "{}").unwrap();
        // A file added: refused.
        std::fs::write(folder.join("solve/extra.recp"), "x").unwrap();
        let e = bind_made().unwrap_err();
        assert!(
            e.contains("R/solve/extra.recp is not in what this process made"),
            "{e}"
        );
        assert!(
            e.contains("R/solve/extra.recp is not in run.json's outputs"),
            "{e}"
        );
        // run.json's outputs that disagree with what was made, which the folder matches: refused.
        let made_now = sealed(&dir);
        let e = bind(
            &dir,
            &folder,
            records(Some(&made_now), None, Some(&outputs)),
            &inputs,
        )
        .unwrap_err();
        assert!(
            e.contains("R/solve/extra.recp is not in run.json's outputs"),
            "{e}"
        );
        assert!(!e.contains("what this process made"), "{e}");
        std::fs::remove_file(folder.join("solve/extra.recp")).unwrap();
        // A file of a record gone: refused.
        let mut more = outputs.clone();
        more.push(crate::run::manifest::OutputRef {
            path: "solve/gone.recp".into(),
            size: 1,
            sha256: "0".repeat(64),
        });
        let e = bind(
            &dir,
            &folder,
            records(Some(&made), None, Some(&more)),
            &inputs,
        )
        .unwrap_err();
        assert!(e.contains("R/solve/gone.recp is gone"), "{e}");
        // What was made naming another run folder: refused.
        let other = SealedRun {
            run_folder: "Q".into(),
            ..made.clone()
        };
        let e = bind(&dir, &folder, records(Some(&other), None, None), &inputs).unwrap_err();
        assert!(
            e.contains("what this process made names the run folder Q"),
            "{e}"
        );
        done(&dir);
    }

    /// M8b: an earlier bed's run bound by its seal. Every file under the run's folder is held to
    /// it, `project.simpa` and `run.json` included; a file changed, added or gone, or a run folder
    /// that is not the one sealed, is refused.
    #[test]
    fn a_run_bound_to_a_seal_is_refused_when_any_file_is_not_its_records() {
        let (dir, folder, inputs) = run_dir("seal");
        let seal = sealed(&dir);
        assert_eq!(seal.files.len(), 8);
        let bind_sealed =
            |s: &SealedRun| bind(&dir, &folder, records(None, Some(s), None), &inputs);
        let b = bind_sealed(&seal).unwrap();
        assert_eq!(b.by, ["seal"]);
        assert_eq!(b.outputs.len(), 2);
        // Both records, when both are there.
        let outputs = hash_tree(&folder, &[RUN_JSON]).unwrap();
        let b = bind(
            &dir,
            &folder,
            records(None, Some(&seal), Some(&outputs)),
            &inputs,
        )
        .unwrap();
        assert_eq!(b.by, ["run.json", "seal"]);
        // run.json edited (its project's sha256, say): refused by the seal.
        std::fs::write(folder.join(RUN_JSON), "{\"edited\": 1}").unwrap();
        let e = bind_sealed(&seal).unwrap_err();
        assert!(e.starts_with(FILES_CHANGED), "{e}");
        assert!(e.contains("R/run.json has 13 bytes"), "{e}");
        std::fs::write(folder.join(RUN_JSON), "{}").unwrap();
        // project.simpa changed: refused.
        std::fs::write(dir.join("project.simpa"), "{\"a\":1}").unwrap();
        let e = bind_sealed(&seal).unwrap_err();
        assert!(e.contains("project.simpa has 7 bytes"), "{e}");
        std::fs::write(dir.join("project.simpa"), "{}").unwrap();
        // An output changed: refused.
        let stats = folder.join("solve/SPPS particle statistics.gabe");
        std::fs::write(&stats, "stats 4").unwrap();
        let e = bind_sealed(&seal).unwrap_err();
        assert!(
            e.contains("R/solve/SPPS particle statistics.gabe has 7 bytes"),
            "{e}"
        );
        std::fs::write(&stats, "stats 3").unwrap();
        assert!(bind_sealed(&seal).is_ok());
        // The seal naming another run folder: refused.
        let other = SealedRun {
            run_folder: "Q".into(),
            ..seal.clone()
        };
        let e = bind_sealed(&other).unwrap_err();
        assert!(e.contains("the seal names the run folder Q"), "{e}");
        // A file added beside the run folder: refused.
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let e = bind_sealed(&seal).unwrap_err();
        assert!(e.contains("notes.txt is not in the seal"), "{e}");
        done(&dir);
    }

    /// M8b: a run that no record binds, whose `run.json` records no output hashes, and which no
    /// seal names, is refused `bed_run_unbound` before anything is read: not judged on its files'
    /// word.
    #[test]
    fn a_run_with_no_output_hashes_and_no_seal_entry_is_refused() {
        let (dir, folder, inputs) = run_dir("unbound");
        let e = bind(&dir, &folder, Records::default(), &inputs).unwrap_err();
        assert!(e.starts_with(RUN_UNBOUND), "{e}");
        assert!(e.contains("not recorded: a run made before M8b"), "{e}");
        // A seal with no entry for the run gives it none.
        let seal = Seal::of_runs("20260929T093134Z", BTreeMap::new());
        let key = super::super::run::RunKey::Cell {
            id: "c".into(),
            seed: 1,
        };
        assert_eq!(seal_key(&key), "runs/c/s1");
        assert!(seal.run(&key).is_none());
        let e = bind(&dir, &folder, records(None, seal.run(&key), None), &inputs).unwrap_err();
        assert!(e.starts_with(RUN_UNBOUND), "{e}");
        done(&dir);
    }

    /// M8b round 2: a seal inside the bed folder it would hold binds nothing (an edit of the bed
    /// can edit it too), and is refused; one outside it is taken.
    #[test]
    fn a_seal_inside_its_bed_folder_is_refused() {
        // `dir` stands for the bed folder here; the test's own folder holds it.
        let (dir, folder, _) = run_dir("inside");
        let own = dir.parent().unwrap().parent().unwrap().parent().unwrap();
        let text = Seal::of_runs("x", BTreeMap::new()).to_json();
        for (at, inside) in [
            (dir.join("outputs-seal.json"), true),
            (folder.join("outputs-seal.json"), true),
            (own.join("outputs-seal.json"), false),
        ] {
            std::fs::write(&at, &text).unwrap();
            let r = seal_outside(&dir, &at);
            assert_eq!(r.is_err(), inside, "{}: {r:?}", at.display());
            if let Err(e) = r {
                assert!(e.contains("is inside the bed folder"), "{e}");
            }
        }
        // A path with a `..` detour that ends in the bed is the same file.
        let detour = folder.join("solve").join("..").join("outputs-seal.json");
        assert!(seal_outside(&dir, &detour).is_err());
        // A seal file that is not there is refused.
        assert!(seal_outside(&dir, &own.join("not-there.json")).is_err());
        done(&dir);
    }

    /// A seal is checked before it is used: its version, every entry, its totals, and the bed it
    /// is of.
    #[test]
    fn a_seal_is_checked_before_it_is_used() {
        let sha = "a".repeat(64);
        let run = SealedRun {
            run_folder: "R".into(),
            files: vec![
                SealedFile("project.simpa".into(), 2, sha.clone()),
                SealedFile("R/run.json".into(), 3, sha.clone()),
            ],
        };
        let good = Seal {
            seal_version: SEAL_VERSION,
            bed: "20260929T093134Z".into(),
            sealed_from: "C:/tmp/bed/20260929T093134Z".into(),
            sealed_utc: "2026-09-30T03:00:00Z".into(),
            tool: "t".into(),
            why: "w".into(),
            provenance: BTreeMap::new(),
            report_json_sha256: sha.clone(),
            summary_json_sha256: sha.clone(),
            files: 2,
            bytes: 5,
            runs: [("runs/c/s1".to_string(), run)].into(),
        };
        assert_eq!(good.check(), Ok(()));
        assert!(good.for_bed(Path::new("C:/x/20260929T093134Z")).is_ok());
        assert!(good.for_bed(Path::new("C:/x/20260930T000000Z")).is_err());
        let with = |f: &dyn Fn(&mut Seal)| {
            let mut s = good.clone();
            f(&mut s);
            s.check()
        };
        assert!(with(&|s| s.seal_version = 2).is_err());
        assert!(with(&|s| s.files = 3).is_err());
        assert!(with(&|s| s.bytes = 6).is_err());
        let entry = |f: SealedFile| {
            with(&|s| {
                let r = s.runs.get_mut("runs/c/s1").unwrap();
                r.files.push(f.clone());
                s.files += 1;
                s.bytes += f.1;
            })
        };
        assert!(entry(SealedFile("R/x".into(), 1, sha.clone())).is_ok());
        assert!(entry(SealedFile("R/x".into(), 1, "A".repeat(64))).is_err());
        assert!(entry(SealedFile("R/../x".into(), 1, sha.clone())).is_err());
        assert!(entry(SealedFile(r"R\x".into(), 1, sha.clone())).is_err());
        assert!(entry(SealedFile("/R/x".into(), 1, sha.clone())).is_err());
        assert!(entry(SealedFile("project.simpa".into(), 1, sha.clone())).is_err());
        // A run folder without its run.json.
        let no_run_json = with(&|s| {
            let r = s.runs.get_mut("runs/c/s1").unwrap();
            r.files.retain(|f| f.0 != "R/run.json");
            s.files -= 1;
            s.bytes -= 3;
        });
        assert!(no_run_json.is_err(), "{no_run_json:?}");
    }

    #[test]
    fn the_working_directory_is_left_out_of_config_xml_and_nothing_else() {
        let a = r#"<configuration workingdirectory="C:\a\"><x y="1"/>"#;
        let b = r#"<configuration workingdirectory="D:\b\c\"><x y="1"/>"#;
        assert_eq!(without_workdir(a), without_workdir(b));
        assert_eq!(
            without_workdir(a),
            r#"<configuration workingdirectory=""><x y="1"/>"#
        );
        let c = r#"<configuration workingdirectory="C:\a\"><x y="2"/>"#;
        assert_ne!(without_workdir(a), without_workdir(c));
        assert_eq!(without_workdir("<c/>"), "<c/>");
    }
}
