//! The inputs that pick and size a band's noise calibration, read by the production code from a
//! real run (the review of `50695f6`, major: mutations that made shipped values 10 to 14 times too
//! optimistic passed every test, since the evidence took the kind of wall from the cell's own
//! configuration and nothing held the report's reading of it to the run's materials).
//!
//! On the committed energetic SPPS run (tutorial 1's box, three materials) with its `config.xml`'s
//! materials rewritten: every face Lambert with scattering 1 and one absorption, 0.2 (uniform
//! Lambert, M8's rooms at 0.2) and 0.5 (above the uniform-Lambert entries' bound), Lambert with
//! three unequal absorptions, specular walls, and Lambert with scattering 0.5. For each,
//! `results::reference` reads the walls from the materials, `SppsResults::walls` and
//! `noise_model` carry them into the band's model, and `params::noise` picks the entry and the
//! structure. Says no through the code: every band read as having one absorption, or the
//! uniform-Lambert entries taken at any absorption, each moves a case to the wrong entry.

mod common;

use std::path::{Path, PathBuf};

use simpa_core::faults::{self, Fault};
use simpa_core::params::noise::{self, Structure, Walls};
use simpa_core::results::{self, SolverResults, reference::Reference, spps::SppsResults};
use simpa_core::run::expect::Expectation;
use simpa_core::schema::SolverKind;

const ENERGETIC: &str = "results/energetic_spps";

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// `text` with the value of every `name="…"` replaced by `value`.
fn set_attribute(text: &str, name: &str, value: &str) -> String {
    let key = format!("{name}=\"");
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find(&key) {
        out.push_str(&rest[..i + key.len()]);
        rest = &rest[i + key.len()..];
        let end = rest.find('"').unwrap();
        out.push_str(value);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// `config` with the `i`-th material of `<surface_absorption_enum>` given, in every band, the
/// absorption, scattering and law `law(i)`.
fn with_materials(config: &str, law: impl Fn(usize) -> (f64, f64, i32)) -> String {
    let start = config.find("<surface_absorption_enum").unwrap();
    let end = config.find("</surface_absorption_enum>").unwrap();
    let block = &config[start..end];
    let mut parts = block.split("<type_surface");
    let mut out = parts.next().unwrap().to_string();
    for (i, p) in parts.enumerate() {
        let (absorb, diffusion, loi) = law(i);
        let p = set_attribute(p, "absorb", &absorb.to_string());
        let p = set_attribute(&p, "diffusion", &diffusion.to_string());
        let p = set_attribute(&p, "loi", &loi.to_string());
        out.push_str("<type_surface");
        out.push_str(&p);
    }
    format!("{}{out}{}", &config[..start], &config[end..])
}

/// The committed run, and a copy of its `solve/` folder whose `config.xml` the cases rewrite.
fn run_and_copy() -> (SppsResults, PathBuf, String) {
    let run = common::fixture(ENERGETIC);
    let r = results::load(&run).unwrap_or_else(|e| panic!("{e}"));
    let SolverResults::Spps(s) = r.data else {
        panic!("an SPPS run")
    };
    // One folder per call: the tests run beside each other in one process.
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let to = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "noise-inputs-{}-{}",
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    copy_dir(&run.join("solve"), &to);
    let config = std::fs::read_to_string(to.join("config.xml")).unwrap();
    (s, to, config)
}

/// The run with its reference read from `solve` after its materials were rewritten by `law`:
/// each band's walls as the report reads them, and the band's noise model's kind of wall and
/// energetic T30's structure.
fn read_walls(
    s: &SppsResults,
    solve: &Path,
    config: &str,
    law: impl Fn(usize) -> (f64, f64, i32),
) -> Vec<((bool, bool, f64), Walls, Structure)> {
    std::fs::write(solve.join("config.xml"), with_materials(config, law)).unwrap();
    let exp = Expectation::read(solve, SolverKind::Spps).unwrap();
    let reference = results::reference::reference(solve, &exp, s.speed_of_sound_m_s, false);
    assert!(
        matches!(reference, Reference::Computed { .. }),
        "{reference:?}"
    );
    let mut s = s.clone();
    *s.reference = reference;
    let names: Vec<&str> = s.sources.iter().map(|x| x.name.as_str()).collect();
    (0..s.total_energy.len())
        .map(|i| {
            let m = s.noise_model(i, &names);
            (s.walls(i), m.walls(), m.structure(3).unwrap())
        })
        .collect()
}

/// The five cases, each with the kind of wall and energetic T30's structure every band must read.
#[allow(clippy::type_complexity)]
fn cases() -> Vec<(&'static str, Box<dyn Fn(usize) -> (f64, f64, i32)>, Walls)> {
    // Unequal, and below the uniform-Lambert bound (0.2) on average, so that a fault reading them
    // as uniform lands in the uniform-Lambert entries.
    let alphas = [0.15, 0.05, 0.1];
    vec![
        (
            "uniform Lambert 0.2",
            Box::new(|_| (0.2, 1.0, 2)),
            Walls::UniformLambert,
        ),
        (
            "uniform Lambert 0.5",
            Box::new(|_| (0.5, 1.0, 2)),
            Walls::Lambert,
        ),
        (
            "Lambert, three absorptions",
            Box::new(move |i| (alphas[i % 3], 1.0, 2)),
            Walls::Lambert,
        ),
        (
            "specular, three absorptions",
            Box::new(move |i| (alphas[i % 3], 0.0, 0)),
            Walls::Other,
        ),
        (
            "Lambert, scattering 0.5",
            Box::new(|_| (0.2, 0.5, 2)),
            Walls::Other,
        ),
    ]
}

#[test]
fn the_kind_of_wall_a_band_is_calibrated_by_is_read_from_its_materials() {
    let (s, solve, config) = run_and_copy();
    let t30 = |w: Walls| noise::calibration::entry(noise::Method::Energetic, 3, w);
    for (name, law, want) in cases() {
        let got = read_walls(&s, &solve, &config, law);
        assert_eq!(got.len(), 2, "{name}: two bands");
        for ((lambert, uniform, a), walls, structure) in got {
            println!(
                "{name}: lambert {lambert}, uniform {uniform}, mean absorption {a:.3}: {walls:?}, T30 {structure:?}"
            );
            assert_eq!(walls, want, "{name}");
            assert_eq!(structure, t30(want).structure, "{name}");
            let want_a = match name {
                "uniform Lambert 0.5" => 0.5,
                "uniform Lambert 0.2" | "Lambert, scattering 0.5" => 0.2,
                _ => a,
            };
            assert!((a - want_a).abs() < 1e-6, "{name}: {a}");
        }
    }
}

#[test]
fn a_band_read_as_uniform_or_above_the_bound_takes_the_wrong_entry() {
    // Says no through the code: each fault moves the case it touches to the uniform-Lambert
    // entries, 0.052 of M7's structure for T30, and leaves a uniform Lambert room where it was.
    let (s, solve, config) = run_and_copy();
    for (fault, moved) in [
        (Fault::UniformAbsorptionForced, "Lambert, three absorptions"),
        (Fault::UniformLambertBoundIgnored, "uniform Lambert 0.5"),
    ] {
        for (name, law, want) in cases()
            .into_iter()
            .filter(|(n, _, _)| *n == moved || *n == "uniform Lambert 0.2")
        {
            let got = faults::with(fault, || read_walls(&s, &solve, &config, law));
            for (_, walls, _) in got {
                if name == moved {
                    assert_eq!(walls, Walls::UniformLambert, "{fault:?} {name}");
                    assert_ne!(walls, want, "{fault:?} {name}");
                } else {
                    assert_eq!(walls, want, "{fault:?} {name}");
                }
            }
        }
    }
}
