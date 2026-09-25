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

/// A value to its precision, or `NE(<why>)` for a refusal.
fn cell(e: &Evaluated, digits: usize, scale: f64) -> String {
    match e {
        Evaluated::Value { value, .. } => format!("{:.*}", digits, value * scale),
        Evaluated::NotEvaluable { not_evaluable: r } => {
            let why = serde_json::to_value(&r.error)
                .ok()
                .and_then(|v| v["why"]["why"].as_str().map(str::to_string))
                .unwrap_or_else(|| r.code.trim_start_matches("params_").to_string());
            format!("NE({why})")
        }
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
