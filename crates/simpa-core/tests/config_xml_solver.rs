//! M3 gate (c), the solver as oracle: configurations written by `core::config_xml` are run by
//! our M1 builds of SPPS and TCR (`$SIMPA_SOLVERS_DIR`, else `target/solvers/bin`), each in a
//! fresh folder under `target/test-runs/config_xml/`, with the folder as the working directory
//! and `config.xml` as the argument. A failing test keeps its folders for inspection, with
//! `_stdout.txt` and `_stderr.txt`, and names them; a passing one removes them
//! (`common/scratch.rs`; `$SIMPA_KEEP_SCRATCH=1` keeps them all).
//!
//! These tests need the solver build, so they are Grace-local, not CI-portable.

#[path = "config_xml_support.rs"]
mod support;

use std::path::Path;

use simpa_core::config_xml::{self, SolverKind, names, scene_mesh};
use simpa_core::formats::{cbin, csbin, gabe};
use simpa_core::schema::Project;
use support::{
    Run, fresh_run_dir, load_project, outputs, repo_file, run_solver, solver_exe, upstream_file,
};

fn exe(solver: SolverKind) -> std::path::PathBuf {
    solver_exe(match solver {
        SolverKind::Spps => "spps.exe",
        SolverKind::Tcr => "classicalTheory.exe",
    })
}

/// Writes the run folder: `config.xml`, the scene mesh (`mesh.cbin`, given or from
/// [`scene_mesh`]), the tetrahedral mesh, and any directivity files.
fn prepare(
    label: &str,
    p: &Project,
    solver: SolverKind,
    variant: Option<&str>,
    mesh: Option<&Path>,
    tetra: &str,
) -> std::path::PathBuf {
    let dir = fresh_run_dir(label);
    config_xml::write_file(p, solver, variant, &dir).unwrap();
    match mesh {
        Some(m) => {
            std::fs::copy(m, dir.join(names::SCENE_MESH)).unwrap();
        }
        None => cbin::write_file(&scene_mesh(p).unwrap(), &dir.join(names::SCENE_MESH)).unwrap(),
    }
    std::fs::copy(repo_file(tetra), dir.join(names::TETRA_MESH)).unwrap();
    let staged = config_xml::directivity_files(p);
    if !staged.is_empty() {
        let d = dir.join(names::DIRECTIVITY_DIR);
        std::fs::create_dir_all(&d).unwrap();
        for f in staged {
            // The rich cube's balloon is upstream's own sample file.
            assert!(f.project_path.ends_with("speaker-test3.txt"));
            std::fs::copy(
                upstream_file("src/spps/tests/speaker-test3.txt"),
                d.join(&f.run_name),
            )
            .unwrap();
        }
    }
    dir
}

/// The checks every run must pass: exit code 0, no `Xml Property ... doesn't exist` line, and
/// the solver's own completion evidence (SPPS prints `End of calculation.`; TCR has no such
/// line, so its `Main results.gabe` and its third step are required instead).
fn assert_clean(run: &Run, solver: SolverKind) {
    let xml = run.xml_property_lines();
    println!(
        "{}: exit {:?}, {} stdout lines, {} 'Xml Property' lines, stderr {} bytes",
        run.dir.display(),
        run.exit,
        run.stdout.lines().count(),
        xml.len(),
        run.stderr.len()
    );
    for l in &xml {
        println!("  {l}");
    }
    assert_eq!(xml.len(), 0, "stdout:\n{}", run.stdout);
    assert_eq!(run.exit, Some(0), "stderr:\n{}", run.stderr);
    match solver {
        SolverKind::Spps => {
            assert!(
                run.has_line("End of calculation."),
                "stdout:\n{}",
                run.stdout
            );
            assert!(run.dir.join(names::SPPS_STATS_FILE).is_file());
        }
        SolverKind::Tcr => {
            assert!(run.has_line("Step 3/3"), "stdout:\n{}", run.stdout);
            assert!(run.dir.join("Main results.gabe").is_file());
        }
    }
}

/// A clean exit is not enough: SPPS exits 0 with every particle lost. So every point receiver
/// (SPPS `Sound level.recp`) or every band row of TCR's `Main results.gabe` must hold only finite
/// values, and some energy.
fn assert_energy(run: &Run, solver: SolverKind, receivers: &[&str]) -> Vec<f64> {
    let mut sums = Vec::new();
    let tables: Vec<std::path::PathBuf> = match solver {
        SolverKind::Spps => receivers
            .iter()
            .map(|r| {
                run.dir
                    .join(names::POINT_RECEIVER_DIR)
                    .join(r)
                    .join(names::POINT_RECEIVER_FILE)
            })
            .collect(),
        SolverKind::Tcr => vec![run.dir.join("Main results.gabe")],
    };
    for t in tables {
        let g = gabe::read_file(&t).unwrap_or_else(|e| panic!("{}: {e}", t.display()));
        let mut values: Vec<f32> = Vec::new();
        for c in &g.columns {
            if c.floats().is_none() {
                continue;
            }
            match solver {
                SolverKind::Spps => values.extend(c.floats().unwrap()),
                // TCR's Global row is NaN by design for some quantities.
                SolverKind::Tcr => values.extend(g.band_series(c.name()).unwrap().bands),
            }
        }
        assert!(!values.is_empty(), "{}", t.display());
        assert!(
            values.iter().all(|v| v.is_finite()),
            "{}: non-finite values",
            t.display()
        );
        let sum: f64 = values.iter().map(|&v| f64::from(v)).sum();
        println!(
            "  {}: {} values, sum {sum:.4e}",
            t.file_name().unwrap().to_string_lossy(),
            values.len()
        );
        assert!(sum > 0.0, "{}: no energy", t.display());
        sums.push(sum);
    }
    sums
}

/// M3 (c): `cube.simpa`, upstream's `cube.cbin` and `cube_mesh.mbin`.
#[test]
fn cube_runs_clean_in_spps_and_tcr() {
    let p = load_project("tests/fixtures/projects/cube.simpa");
    for solver in [SolverKind::Spps, SolverKind::Tcr] {
        let dir = prepare(
            &format!("cube-{solver:?}"),
            &p,
            solver,
            None,
            Some(&repo_file(
                "tests/fixtures/upstream/lib_interface/cube.cbin",
            )),
            "tests/fixtures/upstream/lib_interface/cube_mesh.mbin",
        );
        let run = run_solver(&exe(solver), &dir);
        assert_clean(&run, solver);
        assert_energy(&run, solver, &["Receiver 1"]);
        println!(
            "{solver:?} outputs: {}",
            outputs(&dir, &["config.xml", "mesh.cbin", "tetramesh.mbin"]).len()
        );
    }
}

/// Every conditional attribute: directional and balloon sources, a transmitting material,
/// background noise, a scene receiver, a cutting plane and a fitting zone, on the base project
/// and on its variant. The variant turns the walls from absorption 0.2 into 0.6, so with the
/// same seed SPPS must record less energy at both receivers: the override reached the solver.
#[test]
fn the_rich_cube_runs_clean_in_spps_and_tcr_with_and_without_its_variant() {
    let p = support::rich_cube();
    let mut spps_energy: Vec<Vec<f64>> = Vec::new();
    for variant in [None, Some("Absorbent walls")] {
        for solver in [SolverKind::Spps, SolverKind::Tcr] {
            let label = format!(
                "rich-{solver:?}-{}",
                if variant.is_some() { "variant" } else { "base" }
            );
            let dir = prepare(
                &label,
                &p,
                solver,
                variant,
                None,
                "tests/fixtures/upstream/lib_interface/cube_mesh.mbin",
            );
            let run = run_solver(&exe(solver), &dir);
            assert_clean(&run, solver);
            let energy = assert_energy(&run, solver, &["Receiver 1", "Receiver 2"]);
            if solver == SolverKind::Spps {
                spps_energy.push(energy);
                // The balloon loaded, and the cutting plane and scene receiver wrote results.
                assert!(!run.stdout.contains("File not open"), "{}", run.stdout);
                let out = outputs(&dir, &[]);
                assert!(out.keys().any(|k| k.contains(names::SURFACE_RECEIVER_DIR)));
                assert!(
                    out.keys().any(|k| k.ends_with(names::CUTTING_PLANE_FILE)),
                    "{:?}",
                    out.keys()
                );
                assert!(
                    out.keys().any(|k| k.contains("Loudspeaker")),
                    "per-source echograms"
                );
            }
        }
    }
    let (base, absorbent) = (&spps_energy[0], &spps_energy[1]);
    println!("SPPS receiver energy, base {base:?}, absorbent walls {absorbent:?}");
    assert!(base.iter().zip(absorbent).all(|(b, a)| a < b));
}

/// A cross-check between two readings of `docs/formats/config_xml.md` and
/// `docs/solver-contract.md`: `core::validate`'s export rules, written separately, must find
/// nothing wrong in any run folder this writer prepares, checked before launch as the contract
/// says.
#[test]
fn the_validators_export_rules_accept_every_run_folder_we_write() {
    use simpa_core::validate::{Severity, validate_export};
    let cube = load_project("tests/fixtures/projects/cube.simpa");
    let rich = support::rich_cube();
    let tutorial1 = load_project("tests/fixtures/projects/tutorial1.simpa");
    let cube_mbin = "tests/fixtures/upstream/lib_interface/cube_mesh.mbin";
    let cube_cbin = repo_file("tests/fixtures/upstream/lib_interface/cube.cbin");
    let t1_mbin = "tests/fixtures/upstream/tutorial1/spps/tetramesh.mbin";
    struct Case<'a> {
        label: &'a str,
        project: &'a Project,
        variant: Option<&'a str>,
        /// `None` writes the scene mesh with `scene_mesh`.
        mesh: Option<&'a Path>,
        tetra: &'a str,
    }
    let case = |label, project, variant, mesh, tetra| Case {
        label,
        project,
        variant,
        mesh,
        tetra,
    };
    let mut folders = 0;
    for solver in [SolverKind::Spps, SolverKind::Tcr] {
        let cases = [
            case("cube", &cube, None, Some(cube_cbin.as_path()), cube_mbin),
            case("rich", &rich, None, None, cube_mbin),
            case(
                "rich-variant",
                &rich,
                Some("Absorbent walls"),
                None,
                cube_mbin,
            ),
            case("tutorial1", &tutorial1, None, None, t1_mbin),
        ];
        for Case {
            label,
            project: p,
            variant,
            mesh,
            tetra,
        } in cases
        {
            // The validator reads the variant from the project, as a run would record it.
            let mut p = p.clone();
            p.active_variant = config_xml::resolve_variant(&p, variant).unwrap();
            let dir = prepare(
                &format!("xcheck-{label}-{solver:?}"),
                &p,
                solver,
                variant,
                mesh,
                tetra,
            );
            let issues = validate_export(&p, &dir, solver);
            for i in &issues {
                println!(
                    "  {label} {solver:?}: {} {:?} {} ({})",
                    i.code, i.severity, i.message, i.path
                );
            }
            let errors = issues
                .iter()
                .filter(|i| i.severity == Severity::Error)
                .count();
            assert_eq!(
                errors, 0,
                "{label} {solver:?}: the validator refuses our run folder"
            );
            folders += 1;
        }
    }
    println!("{folders} run folders pass validate_export with 0 errors");
}

/// `tutorial1.simpa`, written and run; and run beside upstream's own GUI-written configuration
/// with the same seed. Every output file must be byte-identical, except the `.csbin` ones (their
/// padding is nondeterministic, M1), which must be identical once decoded.
#[test]
fn tutorial1_runs_clean_and_matches_upstreams_own_configuration() {
    let p = load_project("tests/fixtures/projects/tutorial1.simpa");
    for (solver, kind) in [(SolverKind::Spps, "spps"), (SolverKind::Tcr, "tcr")] {
        let tetra = format!("tests/fixtures/upstream/tutorial1/{kind}/tetramesh.mbin");
        // Labels of equal length, so both runs' paths are equally long (see the MAX_PATH check).
        let ours = prepare(
            &format!("tutorial1-{kind}-ours"),
            &p,
            solver,
            None,
            None,
            &tetra,
        );
        let run = run_solver(&exe(solver), &ours);
        assert_clean(&run, solver);

        // Upstream's configuration, run the M1 gate's way.
        let theirs = fresh_run_dir(&format!("tutorial1-{kind}-them"));
        let fixture = format!("tests/fixtures/upstream/tutorial1/{kind}");
        let wd = config_xml::working_directory(&theirs).unwrap();
        let text = support::read_text(&format!("{fixture}/config.xml")).replace("__RUNDIR__", &wd);
        std::fs::write(theirs.join("config.xml"), text).unwrap();
        for f in ["mesh.cbin", "tetramesh.mbin"] {
            std::fs::copy(repo_file(&format!("{fixture}/{f}")), theirs.join(f)).unwrap();
        }
        let up = run_solver(&exe(solver), &theirs);
        assert_eq!(up.exit, Some(0));
        let up_xml = up.xml_property_lines();
        println!(
            "upstream's {kind} config: {} 'Xml Property' lines {up_xml:?}",
            up_xml.len()
        );
        // docs/formats/config_xml.md, P1: the GUI writes directivities_directory for SPPS only.
        let expected: &[&str] = match solver {
            SolverKind::Spps => &[],
            SolverKind::Tcr => &["Xml Property directivities_directory doesn't exist !"],
        };
        assert_eq!(up_xml, expected);

        let inputs = ["config.xml", "mesh.cbin", "tetramesh.mbin"];
        let (a, b) = (outputs(&ours, &inputs), outputs(&theirs, &inputs));
        // The solvers open plain paths, so MAX_PATH applies: a file whose full path reaches 260
        // characters is skipped with exit 0 and no message. Seen on 2026-09-23 from a deep
        // scratch folder: `Punctual receiver intensity.gabe` at 260 and 261 characters was never
        // written, while its 252- to 257-character siblings were. Fail on that cause by name.
        let longest = |dir: &Path, files: &std::collections::BTreeMap<String, Vec<u8>>| {
            files
                .keys()
                .chain(std::iter::once(
                    &"Punctual receivers/Receiver 1/Punctual receiver intensity.gabe".to_string(),
                ))
                .map(|k| dir.join(k).as_os_str().len())
                .max()
                .unwrap_or(0)
        };
        let longest = longest(&ours, &a).max(longest(&theirs, &b));
        println!("{kind}: longest output path {longest} characters");
        assert!(
            longest < 260,
            "run folders too deep for MAX_PATH (output_path_too_long): move the repo"
        );
        assert_eq!(
            a.keys().collect::<Vec<_>>(),
            b.keys().collect::<Vec<_>>(),
            "{kind}: the same files"
        );
        let mut identical = 0;
        let mut csbin = 0;
        for (k, bytes) in &a {
            if k.ends_with(".csbin") {
                // Padding bytes differ run to run (M1), so compare decoded, bit for bit (the
                // canonical dump spells every float as hex). The receiver's id is the one thing
                // that may differ: upstream's 3503 is our 0 (config and scene mesh alike).
                assert_eq!(bytes.len(), b[k].len(), "{kind} {k}: same size");
                let ours_rs = csbin::read(bytes).unwrap();
                let mut theirs_rs = csbin::read(&b[k]).unwrap();
                for (t, o) in theirs_rs.receivers.iter_mut().zip(&ours_rs.receivers) {
                    assert_eq!((t.xml_index, o.xml_index), (3503, 0), "{kind} {k}");
                    t.xml_index = o.xml_index;
                }
                assert_eq!(
                    csbin::dump(&ours_rs),
                    csbin::dump(&theirs_rs),
                    "{kind} {k} decoded"
                );
                csbin += 1;
            } else {
                assert!(*bytes == b[k], "{kind} {k} differs");
                identical += 1;
            }
        }
        println!(
            "{kind}: {} output files; {identical} byte-identical to upstream's run, {csbin} .csbin \
             identical once decoded (receiver id 3503 -> 0)",
            a.len()
        );
    }
}
