//! The Simulate settings editor's core (docs/investigations/2026-10-03-pq3/PLAN.md, order of
//! work 1 and 2): the validator refusals the editor shows inline, and the load-bearing round
//! trip: every value the editor sets through its op is the attribute written to `config.xml`, and
//! save/load keeps it bit for bit.

use std::path::PathBuf;

use simpa_core::config_xml::write;
use simpa_core::schema::{
    self, AirAbsorption, AttenuationUnit, BandKind, BandSet, ComputationMethod, F64, History, Op,
    Project, SolverKind,
};
use simpa_core::validate::{self, Context, Issue, Severity, codes};

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;

fn cube() -> Project {
    schema::load(&paths::fixture("projects/cube.simpa")).expect("cube.simpa loads")
}

fn with_solver(solver: Option<SolverKind>) -> Context {
    Context {
        solver,
        ..Context::default()
    }
}

fn of<'a>(issues: &'a [Issue], code: &str) -> Vec<&'a Issue> {
    issues.iter().filter(|i| i.code == code).collect()
}

// ---- no_band_computed ---------------------------------------------------------------------------

/// Every band off for the solver that runs blocks the run; the other solver's flags do not, and
/// with no solver named (an edit, `simpa validate`) the rule does not apply.
#[test]
fn every_band_off_for_the_chosen_solver_blocks_the_run_and_only_that_solver() {
    for (off, other) in [
        (SolverKind::Spps, SolverKind::Tcr),
        (SolverKind::Tcr, SolverKind::Spps),
    ] {
        let mut p = cube();
        let flags = match off {
            SolverKind::Spps => &mut p.solvers.spps.bands_computed,
            SolverKind::Tcr => &mut p.solvers.tcr.bands_computed,
        };
        flags.iter_mut().for_each(|f| *f = false);
        let field = match off {
            SolverKind::Spps => "/solvers/spps/bands_computed",
            SolverKind::Tcr => "/solvers/tcr/bands_computed",
        };

        let chosen = validate::validate_with(&p, &with_solver(Some(off)));
        let hits = of(&chosen, codes::NO_BAND_COMPUTED);
        assert_eq!(hits.len(), 1, "{off:?}: {chosen:#?}");
        assert_eq!(hits[0].severity, Severity::Error);
        assert_eq!(hits[0].path, field);
        assert!(validate::has_errors(&chosen));
        assert_eq!(chosen.len(), 1, "nothing else: {chosen:#?}");

        // The other solver computes every band: it runs.
        assert_eq!(
            validate::validate_with(&p, &with_solver(Some(other))),
            Vec::new(),
            "{other:?} is not blocked by {off:?}'s flags"
        );
        // No solver named: the rule is about a run, so it does not apply.
        assert_eq!(validate::validate(&p), Vec::new());
        // The solver-scoped rules alone, as the app asks for them per solver.
        assert_eq!(validate::solver_issues(&p, off), chosen);
        assert_eq!(validate::solver_issues(&p, other), Vec::new());
    }
}

/// One band on is enough, whichever it is.
#[test]
fn one_band_computed_runs() {
    for band in 0..6 {
        let mut p = cube();
        p.solvers.spps.bands_computed = (0..6).map(|j| j == band).collect();
        assert_eq!(
            validate::validate_with(&p, &with_solver(Some(SolverKind::Spps))),
            Vec::new(),
            "band {band}"
        );
    }
}

// ---- the air ------------------------------------------------------------------------------------

/// Non-physical air is refused (`atmosphere_invalid`): humidity outside 0-100 %, pressure at or
/// below 0, temperature at or below absolute zero. The edges that are physical pass.
#[test]
fn non_physical_air_is_refused() {
    let set = |t: f64, h: f64, pa: f64| {
        let mut p = cube();
        p.environment.temperature_c = F64::new(t);
        p.environment.relative_humidity_percent = F64::new(h);
        p.environment.pressure_pa = F64::new(pa);
        validate::validate(&p)
    };
    for (t, h, pa, field) in [
        (20.0, 100.5, 101_325.0, "relative_humidity_percent"),
        (20.0, -0.1, 101_325.0, "relative_humidity_percent"),
        (20.0, 50.0, 0.0, "pressure_pa"),
        (20.0, 50.0, -1.0, "pressure_pa"),
        (-273.15, 50.0, 101_325.0, "temperature_c"),
        (-300.0, 50.0, 101_325.0, "temperature_c"),
    ] {
        let issues = set(t, h, pa);
        let bad = of(&issues, codes::ATMOSPHERE_INVALID);
        assert_eq!(bad.len(), 1, "{t} °C {h} % {pa} Pa: {issues:#?}");
        assert_eq!(bad[0].path, format!("/environment/{field}"));
        assert_eq!(bad[0].severity, Severity::Error);
    }
    for (t, h) in [(20.0, 0.0), (20.0, 100.0)] {
        let issues = set(t, h, 101_325.0);
        assert!(!validate::has_errors(&issues), "{t} °C {h} %: {issues:#?}");
    }
}

/// Outside the range where ISO 9613-1 (the formula both solvers use for air absorption) states
/// an accuracy, the project is warned, never refused. Inside it, nothing.
#[test]
fn air_outside_the_formula_s_range_is_a_warning_not_a_refusal() {
    let air = |t: f64, h: f64, pa: f64| {
        let mut p = cube();
        p.environment.temperature_c = F64::new(t);
        p.environment.relative_humidity_percent = F64::new(h);
        p.environment.pressure_pa = F64::new(pa);
        p
    };
    for (t, h, pa) in [
        (60.0, 50.0, 101_325.0),  // above +50 °C
        (-30.0, 50.0, 101_325.0), // below -20 °C, h above 0.005 %
        (20.0, 50.0, 250_000.0),  // at or above 200 kPa
    ] {
        let issues = validate::validate(&air(t, h, pa));
        let warned = of(&issues, codes::ATMOSPHERE_OUTSIDE_FORMULA_RANGE);
        assert_eq!(warned.len(), 1, "{t} °C {h} % {pa} Pa: {issues:#?}");
        assert_eq!(warned[0].severity, Severity::Warning);
        assert_eq!(warned[0].path, "/environment");
        assert!(
            !validate::has_errors(&issues),
            "a warning only: {issues:#?}"
        );
    }
    for (t, h, pa) in [
        (20.0, 50.0, 101_325.0),
        // Clause 7's edges are inside: -20 °C is 253.14999999999998 K in f64, so the range is
        // checked in °C (`params::air::stated_accuracy`).
        (-20.0, 50.0, 101_325.0),
        (50.0, 10.0, 101_325.0),
        (20.0, 0.0, 101_325.0), // h below 0.005 %: clause 7.3's ±50 % still applies
    ] {
        let issues = validate::validate(&air(t, h, pa));
        assert_eq!(issues, Vec::new(), "{t} °C {h} % {pa} Pa");
    }
    // The formula is not used: a user-set absorption, or air absorption off in both solvers.
    let mut p = air(60.0, 50.0, 101_325.0);
    p.environment.air_absorption = AirAbsorption::UserDefined {
        value: F64::new(0.01),
        unit: AttenuationUnit::DecibelPerMetre,
    };
    assert_eq!(validate::validate(&p), Vec::new());
    let mut p = air(60.0, 50.0, 101_325.0);
    p.solvers.spps.air_absorption = false;
    p.solvers.tcr.air_absorption = false;
    assert_eq!(validate::validate(&p), Vec::new());
    // Non-physical air is the refusal's, not the warning's too.
    let issues = validate::validate(&air(20.0, 120.0, 101_325.0));
    assert!(of(&issues, codes::ATMOSPHERE_OUTSIDE_FORMULA_RANGE).is_empty());
}

// ---- the round trip (order of work 2) -----------------------------------------------------------

fn workdir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\runs\pq3")
    } else {
        PathBuf::from("/runs/pq3")
    }
}

/// `element@attribute` of a config, and the `docalc` of each band in `freq_enum`.
struct Config {
    sim: std::collections::BTreeMap<String, String>,
    atmo: std::collections::BTreeMap<String, String>,
    bands: Vec<(u32, String)>,
}

fn config(p: &Project, solver: SolverKind) -> Config {
    let xml = write(p, solver, None, &workdir()).unwrap_or_else(|e| panic!("{e}"));
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let el = |name: &str| {
        doc.descendants()
            .find(|n| n.has_tag_name(name))
            .unwrap_or_else(|| panic!("<{name}>"))
    };
    let attrs = |n: roxmltree::Node| {
        n.attributes()
            .map(|a| (a.name().to_string(), a.value().to_string()))
            .collect()
    };
    let bands = el("freq_enum")
        .children()
        .filter(|c| c.is_element())
        .map(|b| {
            (
                b.attribute("freq").unwrap().parse().unwrap(),
                b.attribute("docalc").unwrap().to_string(),
            )
        })
        .collect();
    Config {
        sim: attrs(el("simulation")),
        atmo: attrs(el("condition_atmospherique")),
        bands,
    }
}

/// A real attribute is the value's shortest round-trip text, so it reads back to the same bits.
fn assert_real(attrs: &std::collections::BTreeMap<String, String>, key: &str, v: f64) {
    let text = &attrs[key];
    assert_eq!(text, &format!("{v}"), "{key}");
    assert_eq!(text.parse::<f64>().unwrap().to_bits(), v.to_bits(), "{key}");
}

/// Applies `op` as the app does (JSON text through the exact reader, then the history), and
/// checks that save/load keeps the project bit for bit.
fn apply(p: &mut Project, h: &mut History, op: Op) {
    let op = Op::from_json(&op.to_json()).unwrap();
    h.apply(p, op).unwrap();
    let text = schema::to_json(p);
    let back = schema::from_json(&text).unwrap();
    assert_eq!(&back, p, "save/load keeps the project");
    assert_eq!(schema::to_json(&back), text, "byte for byte");
}

fn temp_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("simpa-pq3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn saved_and_loaded(p: &Project, name: &str) -> Project {
    let file = temp_file(name);
    schema::save(p, &file).unwrap();
    let back = schema::load(&file).unwrap();
    std::fs::remove_file(&file).ok();
    back
}

/// Every field the editor sets (PLAN.md scope: C7, C8, C10, C11, C12, C21, C22, C25, C26, C27)
/// reaches `config.xml` as the value set, and a project file keeps it bit for bit.
#[test]
fn every_editable_field_reaches_config_xml_and_survives_save_and_load() {
    let original = cube();
    let mut p = original.clone();
    let mut h = History::new();

    // C7, C8, C10, C11, C12, C21, C22: one SetSolverSettings, as the editor sends it.
    let mut s = p.solvers.clone();
    s.spps.particles_per_source = 123_457;
    s.spps.particles_saved = 17;
    s.spps.duration_s = F64::new(1.7);
    s.spps.time_step_s = F64::new(0.0013);
    s.spps.method = ComputationMethod::Energetic;
    s.spps.sound_maps_per_band = false;
    s.spps.echogram_per_source = true;
    apply(&mut p, &mut h, Op::SetSolverSettings { settings: s });

    // C25: one band off per solver, different bands.
    apply(
        &mut p,
        &mut h,
        Op::SetBandComputed {
            solver: SolverKind::Spps,
            band: 1,
            computed: false,
        },
    );
    apply(
        &mut p,
        &mut h,
        Op::SetBandComputed {
            solver: SolverKind::Tcr,
            band: 4,
            computed: false,
        },
    );

    // C27: temperature, humidity, pressure.
    let mut env = p.environment.clone();
    env.temperature_c = F64::new(23.4);
    env.relative_humidity_percent = F64::new(61.5);
    env.pressure_pa = F64::new(98_765.4);
    apply(&mut p, &mut h, Op::SetEnvironment { environment: env });

    let spps = config(&p, SolverKind::Spps);
    assert_eq!(spps.sim["nbparticules"], "123457");
    assert_eq!(spps.sim["nbparticules_rendu"], "17");
    assert_real(&spps.sim, "duree_simulation", 1.7);
    assert_real(&spps.sim, "pasdetemps", 0.0013);
    assert_eq!(spps.sim["computation_method"], "1");
    assert_eq!(spps.sim["output_recs_byfreq"], "0");
    assert_eq!(spps.sim["output_recp_bysource"], "1");
    fn docalc(c: &Config) -> Vec<&str> {
        c.bands.iter().map(|(_, d)| d.as_str()).collect()
    }
    assert_eq!(docalc(&spps), ["1", "0", "1", "1", "1", "1"]);
    let tcr = config(&p, SolverKind::Tcr);
    assert_eq!(docalc(&tcr), ["1", "1", "1", "1", "0", "1"]);
    for c in [&spps, &tcr] {
        assert_real(&c.atmo, "temperature", 23.4);
        assert_real(&c.atmo, "humidite", 61.5);
        assert_real(&c.atmo, "pression", 98_765.4);
    }

    // Random, the other method, is code 0.
    let mut s = p.solvers.clone();
    s.spps.method = ComputationMethod::Random;
    apply(&mut p, &mut h, Op::SetSolverSettings { settings: s });
    assert_eq!(config(&p, SolverKind::Spps).sim["computation_method"], "0");

    // C26: a preset, through `Project::rebanded` as `edit_reband` applies it.
    let thirds = BandSet::range(BandKind::ThirdOctave, 100, 5000).unwrap();
    let op = p.rebanded(thirds.clone()).unwrap();
    apply(&mut p, &mut h, op);
    let spps = config(&p, SolverKind::Spps);
    let freqs: Vec<u32> = spps.bands.iter().map(|(f, _)| *f).collect();
    assert_eq!(freqs, thirds.frequencies_hz);
    // 250 Hz was off: its three thirds (200, 250, 315) are off; everything else is on.
    for (f, d) in &spps.bands {
        let want = if [200, 250, 315].contains(f) {
            "0"
        } else {
            "1"
        };
        assert_eq!(d, want, "{f} Hz");
    }

    // Through a real file too.
    let back = saved_and_loaded(&p, "settings.simpa");
    assert_eq!(back, p);

    // And every edit undoes to the original, bit for bit.
    while h.undo(&mut p).unwrap() {}
    assert_eq!(p, original);
    assert_eq!(schema::to_json(&p), schema::to_json(&original));
}

/// The values the editor shows for an import are the import's own: opening and saving a project
/// changes none of them (decision rows 11, 41, 43).
#[test]
fn an_imports_own_values_are_kept() {
    let p = cube();
    let back = saved_and_loaded(&p, "import.simpa");
    assert_eq!(back.solvers, p.solvers);
    assert_eq!(back.environment, p.environment);
    assert_eq!(back.bands, p.bands);
    assert_eq!(back.solvers.spps.extinction_exponent, F64::new(5.0));
}

/// PQ3 call 3: a new project writes an echogram per source (the probe showed the flag changes
/// no other output: BUILD.md); particles saved stay 0 (call 2) and the extinction exponent 7
/// (call 1, row 41).
#[test]
fn a_new_project_writes_an_echogram_per_source() {
    let p = Project::new("new");
    assert!(p.solvers.spps.echogram_per_source);
    assert_eq!(p.solvers.spps.particles_saved, 0);
    assert_eq!(p.solvers.spps.extinction_exponent, F64::new(7.0));
    assert!(p.solvers.spps.sound_maps_per_band);
}
