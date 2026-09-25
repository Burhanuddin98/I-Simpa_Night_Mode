//! `simpa results <run-folder> [--json]` and `simpa results --schema` (milestone M7;
//! `docs/formats/results-json.md`).
//!
//! Exit codes (`docs/formats/results-json.md`, "Command and exit codes"): 0 the run's results
//! were read; 2 a usage error, or a path that is not a folder; 5 the run is FAIL, CRASH or
//! CANCELLED, a solver run that did not succeed (the command itself was not cancelled, so not 130);
//! 6 its results do not verify (`core::results`' other refusals). A refusal is printed on stderr
//! as `simpa: results refused: <code>: <detail>`, and with `--json` also on stdout as JSON.

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use simpa_core::results::report::{self, Evaluated, ReferenceReport, RefusalReport, Report};
use simpa_core::results::{self};

use crate::fail;

/// The two schemas `--schema` prints, the file `docs/formats/results-json.schema.json` holds.
pub fn schemas() -> serde_json::Value {
    serde_json::json!({
        "report": report::report_schema(),
        "refusal": report::refusal_schema(),
    })
}

pub fn results_cmd(args: &[&str]) -> ExitCode {
    let mut json = false;
    let mut schema = false;
    let mut folder = None;
    for &a in args {
        match a {
            "--json" => json = true,
            "--schema" => schema = true,
            _ if a.starts_with("--") => return fail(&format!("unknown option '{a}'")),
            _ if folder.is_none() => folder = Some(a),
            _ => return fail(&format!("unexpected argument '{a}'")),
        }
    }
    if schema {
        if folder.is_some() || json {
            return fail("results --schema takes nothing else");
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&schemas()).expect("a schema serialises")
        );
        return ExitCode::SUCCESS;
    }
    let Some(folder) = folder else {
        return fail("results needs a run folder: simpa results <run-folder> [--json]");
    };
    let folder = Path::new(folder);
    if !folder.is_dir() {
        return fail(&format!("{} is not a folder", folder.display()));
    }
    match results::load(folder).and_then(|r| results::checked_report(&r)) {
        Ok(rep) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&rep).expect("a report serialises")
                );
            } else {
                print!("{}", text(&rep));
            }
            ExitCode::SUCCESS
        }
        Err(refusal) => {
            eprintln!("simpa: results refused: {refusal}");
            let code = refusal.exit_code();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&RefusalReport::new(folder, refusal))
                        .expect("a refusal serialises")
                );
            }
            ExitCode::from(code)
        }
    }
}

/// A value to its precision, or `NE(<why>)` for a refusal; for a refusal for its Monte-Carlo
/// noise, `NE(noise:<count>)` with the particles per source that would bring it within its limit
/// ([`particles`]), or `NE(noise)` when none can be named; for a run outside the noise model's
/// calibration, `NE(uncal:<count>)` with the particles per source that reach it, or
/// `NE(uncal:R<=<s>x)` with the most the receiver radius may be as a multiple of the run's.
fn cell(e: &Evaluated, digits: usize, scale: f64) -> String {
    match e {
        Evaluated::Value { value, .. } => format!("{:.*}", digits, value * scale),
        Evaluated::NotEvaluable { not_evaluable: r } => {
            let error = serde_json::to_value(&r.error).unwrap_or_default();
            let why = error["why"]["why"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| r.code.trim_start_matches("params_").to_string());
            if why == "monte_carlo_noise" {
                return match error["why"]["particle_count"]["particles"].as_u64() {
                    Some(n) => format!("NE(noise:{})", particles(n)),
                    None => "NE(noise)".into(),
                };
            }
            if why == "noise_uncalibrated" {
                let w = &error["why"];
                return match (
                    w["particles_at_least"].as_u64(),
                    w["receiver_radius_scale_at_most"].as_f64(),
                ) {
                    (Some(n), _) => format!("NE(uncal:{})", particles(n)),
                    // At most: never shown larger than it is.
                    (None, Some(s)) => format!("NE(uncal:R<={:.2}x)", (s * 100.0).floor() / 100.0),
                    (None, None) => "NE(uncal)".into(),
                };
            }
            format!("NE({why})")
        }
    }
}

/// A particle count in few characters: `150k`, `2.4M`, `1.3G`.
fn particles(n: u64) -> String {
    let n = n as f64;
    let (v, unit) = if n >= 1e9 {
        (n / 1e9, "G")
    } else if n >= 1e6 {
        (n / 1e6, "M")
    } else if n >= 1e3 {
        (n / 1e3, "k")
    } else {
        (n, "")
    };
    // Two significant digits, as the count is rounded; never shown smaller than it is.
    let digits = if v >= 10.0 { 0 } else { 1 };
    let shown = format!("{v:.digits$}");
    let back: f64 = shown.parse().unwrap_or(v);
    if back + 1e-9 < v {
        let step = if digits == 0 { 1.0 } else { 0.1 };
        format!("{:.digits$}{unit}", back + step)
    } else {
        format!("{shown}{unit}")
    }
}

/// The human-readable form: one table per receiver.
fn text(rep: &Report) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "UNVALIDATED: M8's physics bed has not passed; these numbers are not for publication."
    );
    let _ = writeln!(
        s,
        "{} run {} ({}), bands {:?}",
        serde_json::to_value(rep.solver)
            .unwrap()
            .as_str()
            .unwrap_or(""),
        rep.run_folder,
        rep.started,
        rep.bands_hz
    );
    if let Some(sp) = &rep.spps {
        let _ = writeln!(
            s,
            "NE(<why>): not evaluable, and why. NE(noise:<count>): refused for its Monte-Carlo \
             noise; <count> particles per source would bring it within its limit (NE(noise): \
             no count is named, the JSON says why). NE(uncal:<count>) or NE(uncal:R<=<s>x): \
             the run is outside what the noise model was calibrated on; run <count> particles \
             per source, or make the receiver radius at most <s> times this run's."
        );
        for r in &sp.point_receivers {
            let arrival = r
                .arrival_s
                .map_or("detected".to_string(), |t| format!("{:.2} ms", t * 1e3));
            let _ = writeln!(s, "\nreceiver {:?}, arrival {arrival}", r.label);
            let _ = writeln!(
                s,
                "{:>8} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
                "band", "SPL dB", "EDT s", "T20 s", "T30 s", "C50 dB", "C80 dB", "D50 %", "Ts ms"
            );
            let rows = r
                .bands
                .iter()
                .map(|b| (format!("{} Hz", b.freq_hz), &b.parameters))
                .chain([("aggregate".to_string(), &r.aggregate.parameters)])
                .chain(r.per_source.iter().flat_map(|src| {
                    src.bands
                        .iter()
                        .map(move |b| (format!("{} {} Hz", src.source, b.freq_hz), &b.parameters))
                        .chain([(
                            format!("{} aggregate", src.source),
                            &src.aggregate.parameters,
                        )])
                }));
            for (label, p) in rows {
                let _ = writeln!(
                    s,
                    "{label:>8} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
                    cell(&p.spl_db, 1, 1.0),
                    cell(&p.edt_s, 2, 1.0),
                    cell(&p.t20_s, 2, 1.0),
                    cell(&p.t30_s, 2, 1.0),
                    cell(&p.c50_db, 1, 1.0),
                    cell(&p.c80_db, 1, 1.0),
                    cell(&p.d50, 1, 100.0),
                    cell(&p.ts_s, 1, 1000.0),
                );
            }
        }
        match &sp.reference {
            ReferenceReport::Computed {
                volume_m3,
                area_m2,
                free_paths,
                bands,
                ..
            } => {
                let paths = free_paths.as_ref().map_or("refused".to_string(), |p| {
                    format!(
                        "gamma^2 {:.4} ± {:.4}, mean free path {:.3} m (4V/S {:.3} m)",
                        p.gamma2(),
                        p.gamma2_se(),
                        p.mean_free_path_m(),
                        p.four_v_over_s_m()
                    )
                });
                let _ = writeln!(
                    s,
                    "\nreference, analytic, diffuse field, NOT VALIDATED: V {volume_m3:.2} m3, S \
                     {area_m2:.2} m2, {paths}"
                );
                let _ = writeln!(
                    s,
                    "{:>8} {:>11} {:>11} {:>14}",
                    "band", "Kuttruff s", "Eyring s", "Lambert walls"
                );
                for b in bands {
                    let _ = writeln!(
                        s,
                        "{:>8} {:>11} {:>11} {:>14}",
                        format!("{} Hz", b.freq_hz),
                        cell(&b.kuttruff_s, 3, 1.0),
                        cell(&b.eyring_s, 3, 1.0),
                        if b.lambert_walls { "yes" } else { "no" }
                    );
                }
            }
            ReferenceReport::NotComputed { why } => {
                let _ = writeln!(s, "\nreference: not computed: {why}");
            }
        }
    }
    if let Some(t) = &rep.tcr {
        let _ = writeln!(
            s,
            "\n{:>8} {:>10} {:>9} {:>10} {:>9}",
            "band", "A Sab m2", "TR Sab s", "A Eyr m2", "TR Eyr s"
        );
        for b in &t.bands {
            let _ = writeln!(
                s,
                "{:>8} {:>10.2} {:>9.3} {:>10.2} {:>9.3}",
                format!("{} Hz", b.freq_hz),
                b.sabine.absorption_area_m2,
                b.sabine.reverberation_time_s,
                b.eyring.absorption_area_m2,
                b.eyring.reverberation_time_s
            );
        }
        for r in &t.point_receivers {
            let _ = writeln!(s, "\nreceiver {:?}", r.label);
            for b in &r.bands {
                let _ = writeln!(
                    s,
                    "{:>8} direct {:>6.1} dB, total Sabine {:>6.1} dB, total Eyring {:>6.1} dB",
                    format!("{} Hz", b.freq_hz),
                    b.direct_db,
                    b.total_sabine_db,
                    b.total_eyring_db
                );
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use simpa_core::params::{NotEvaluable, ParamError, ParticleCount, Quantity};

    fn noise(count: ParticleCount) -> Evaluated {
        let e = ParamError::NotEvaluable {
            quantity: Quantity::T20,
            why: NotEvaluable::MonteCarloNoise {
                value: 0.8,
                sd: Some(0.05),
                limit: 0.025,
                resamples: 200,
                refused_resamples: 0,
                particle_count: count,
            },
        };
        // The text output reads a refusal as the report holds it.
        Evaluated::NotEvaluable {
            not_evaluable: report::Refused {
                code: e.code().to_string(),
                message: e.to_string(),
                error: e,
            },
        }
    }

    #[test]
    fn a_refusal_for_noise_shows_the_count_it_names() {
        let named = |particles| ParticleCount::Named {
            factor: 16.0,
            margin: 1.4,
            particles,
        };
        assert_eq!(
            cell(&noise(named(Some(2_400_000))), 2, 1.0),
            "NE(noise:2.4M)"
        );
        assert_eq!(cell(&noise(named(Some(150_000))), 2, 1.0), "NE(noise:150k)");
        // A refusal by its resamples names the multiple at which they clear (R4-3).
        let resampled = |particles| ParticleCount::Resampled {
            multiple: 16,
            margin: 1.3,
            particles,
        };
        assert_eq!(
            cell(&noise(resampled(Some(2_400_000))), 2, 1.0),
            "NE(noise:2.4M)"
        );
        // Says no: without a count, none is shown, whatever the reason.
        for c in [
            named(None),
            resampled(None),
            ParticleCount::BeyondResampled { multiple: 64 },
            ParticleCount::ResampledNotConfirmed,
            ParticleCount::ScalingNotConfirmed,
            ParticleCount::NoStandardDeviation,
        ] {
            assert_eq!(cell(&noise(c), 2, 1.0), "NE(noise)");
        }
    }

    #[test]
    fn a_run_outside_the_calibration_shows_what_would_bring_it_inside() {
        let refusal = |at_least, scale| {
            let e = ParamError::NotEvaluable {
                quantity: Quantity::T30,
                why: NotEvaluable::NoiseUncalibrated {
                    value: 0.8,
                    particles: 2_000,
                    crossings_per_particle: 3.0,
                    min_particles: 50_000,
                    max_crossings_per_particle: 2.16,
                    particles_at_least: at_least,
                    receiver_radius_scale_at_most: scale,
                },
            };
            Evaluated::NotEvaluable {
                not_evaluable: report::Refused {
                    code: e.code().to_string(),
                    message: e.to_string(),
                    error: e,
                },
            }
        };
        assert_eq!(cell(&refusal(Some(50_000), None), 2, 1.0), "NE(uncal:50k)");
        // At most: 0.8485 is shown 0.84, never 0.85.
        assert_eq!(
            cell(&refusal(None, Some((2.16f64 / 3.0).sqrt())), 2, 1.0),
            "NE(uncal:R<=0.84x)"
        );
        // Says no: with neither, nothing is claimed.
        assert_eq!(cell(&refusal(None, None), 2, 1.0), "NE(uncal)");
    }

    #[test]
    fn a_count_is_shortened_but_never_shown_smaller_than_it_is() {
        for (n, want) in [
            (999, "999"),
            (150_000, "150k"),
            (160_000, "160k"),
            (1_300_000, "1.3M"),
            (1_250_000, "1.3M"),
            (12_000_000, "12M"),
            (12_500_000, "13M"),
            (2_100_000_000, "2.1G"),
        ] {
            assert_eq!(particles(n), want, "{n}");
        }
    }
}
