//! The M12c beds of the numbers parity added (`docs/investigations/2026-10-09-parity-refresh/
//! README.md` section 4; decision 77 for the maps): each new number read from synthetic series
//! whose answer is known in closed form, **through the code the report uses**
//! (`report::parameters_with`, the same path every SPPS series of `report_with` takes: the decay
//! analysis, the noise judgement and the shown range), and held to a stated tolerance. A bed
//! passes only when every case holds; each bed also carries controls, a wrong reference the
//! same comparison must catch, so a check that cannot fail does not pass.
//!
//! The references are written here from their closed forms, not from the code under test:
//! - an exponential decay `E(t) = e^{−t/τ}` has a straight Schroeder curve, `−60·t/T` dB, so
//!   every range gives `T`;
//! - a two-slope decay `A·e^{−t/τ₁} + B·e^{−t/τ₂}` has the Schroeder integral
//!   `A·τ₁·e^{−t/τ₁} + B·τ₂·e^{−t/τ₂}`; a range's `T` is `−60` over the least-squares slope of its
//!   level between the times it crosses the range's ends, the integrals by Simpson's rule on
//!   20,000 steps;
//! - a direct sound `D` at 0 and a reverberation `R·e^{−t/τ}` split at `te` as
//!   `C = 10·lg((D + Rτ(1 − e^{−te/τ})) / (Rτ·e^{−te/τ}))`, `D_te = (D + Rτ(1 − e^{−te/τ})) /
//!   (D + Rτ)` (ISO 3382-1 A.2.3, Eqs. A.10-A.12);
//! - impulses either side of `te` split as their energies do.
//!
//! The histograms are each bin's exact integral of the continuous decay (an impulse in the bin it
//! falls in). Every series is complete (nothing after its end) and the noise model's deposit is
//! negligible, so the only error left is the computation's.

use schemars::JsonSchema;
use serde::Serialize;

use super::report::{Evaluated, parameters_with};
use crate::params::EnergySeries;
use crate::params::decay::{Arrival, Extra};
use crate::params::noise::{Method, NoiseModel};

/// One case of a bed: a number against its closed form.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Case {
    /// What the series is and which number is read.
    pub name: String,
    /// The number's JSON name (`t15_s`, `t40_s`, `c30_db`, `d80`).
    pub quantity: String,
    /// The closed form it is held to, in words.
    pub reference: String,
    /// The closed form's value; for a refusal, NaN is not used: `expected_refusal` names it.
    pub expected: Option<f64>,
    /// The refusal the case expects (`range_not_reached`), instead of a value.
    pub expected_refusal: Option<String>,
    /// What the report path gave: the shown value, or `None` when it refused.
    pub value: Option<f64>,
    /// The shown value's status (`ok`, `wide`), or the refusal's kind.
    pub status: String,
    /// The largest error allowed, in `unit`; relative when `relative`.
    pub tolerance: f64,
    pub relative: bool,
    pub unit: String,
    /// `|value − expected|`, relative when `relative`; `None` for a refusal case.
    pub error: Option<f64>,
    /// A control: a wrong reference, which the comparison must catch (`holds` then means it was
    /// caught).
    pub control: bool,
    pub holds: bool,
}

/// One bed's outcome, as `simpa bed-extra` writes it.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct BedRun {
    /// `m12c-r15`, ...
    pub bed: String,
    /// What the bed tests, in words.
    pub what: String,
    /// The rule it passes by.
    pub rule: String,
    pub pass: bool,
    /// Cases that do not hold, controls included.
    pub failures: usize,
    /// The largest error of the non-control value cases, relative or absolute as each states.
    pub worst: Option<Worst>,
    pub cases: Vec<Case>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Worst {
    pub case: String,
    pub error: f64,
    pub tolerance: f64,
    pub unit: String,
}

const RULE: &str = "PASS when every case holds: each value within its tolerance of the closed \
form (1/10 of the quantity's difference limen: 0.5 % for a decay time, 0.1 dB for C, 0.005 for \
D), each expected refusal refused with that kind, and each control (a reference moved by twice \
the tolerance) caught";

fn bed(bed: &str, what: &str, cases: Vec<Case>) -> BedRun {
    let failures = cases.iter().filter(|c| !c.holds).count();
    let worst = cases
        .iter()
        .filter(|c| !c.control)
        .filter_map(|c| {
            c.error.map(|e| Worst {
                case: c.name.clone(),
                error: e,
                tolerance: c.tolerance,
                unit: c.unit.clone(),
            })
        })
        .max_by(|a, b| (a.error / a.tolerance).total_cmp(&(b.error / b.tolerance)));
    BedRun {
        bed: bed.into(),
        what: what.into(),
        rule: RULE.into(),
        pass: failures == 0,
        failures,
        worst,
        cases,
    }
}

/// A noise model whose deposit is negligible beside every bin: the value's range is its own.
fn quiet() -> NoiseModel {
    NoiseModel::crossings(1e-18, Method::Random, None).expect("a positive deposit")
}

/// `τ` of a decay that falls 60 dB in `t60` s.
fn tau(t60: f64) -> f64 {
    t60 / (6.0 * std::f64::consts::LN_10)
}

/// Each bin's exact integral of `a·e^{−t/τ}` for `n` bins of `dt`.
fn exp_bins(a: f64, tau: f64, dt: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|k| a * tau * ((-(k as f64) * dt / tau).exp() - (-((k + 1) as f64) * dt / tau).exp()))
        .collect()
}

/// The value or refusal of `name` in the parameters of `series` from `arrival`, with `custom`.
fn read(series: &EnergySeries, arrival: Arrival, custom: &[Extra], name: &str) -> Evaluated {
    let (p, _) = parameters_with(&Ok(series.clone()), arrival, &quiet(), custom);
    match name {
        "t15_s" => p.t15_s,
        "t20_s" => p.t20_s,
        "t30_s" => p.t30_s,
        "c50_db" => p.c50_db,
        "c80_db" => p.c80_db,
        "d50" => p.d50,
        other => {
            p.custom
                .into_iter()
                .find(|c| c.name == other)
                .unwrap_or_else(|| panic!("{other} was not asked for"))
                .value
        }
    }
}

fn status_of(e: &Evaluated) -> String {
    match e {
        Evaluated::Value { status, .. } => status.map_or_else(
            || "value".into(),
            |s| {
                serde_json::to_value(s)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default()
            },
        ),
        Evaluated::NotEvaluable { not_evaluable } => {
            serde_json::to_value(not_evaluable.error.not_evaluable())
                .ok()
                .and_then(|v| v.get("why").and_then(|w| w.as_str()).map(String::from))
                .unwrap_or_else(|| not_evaluable.code.clone())
        }
    }
}

/// What a value case checks.
struct Want<'a> {
    name: String,
    quantity: &'a str,
    reference: String,
    expected: f64,
    tolerance: f64,
    relative: bool,
    unit: &'a str,
}

/// A value case and its control (the reference moved by twice the tolerance, which must fail).
fn value_cases(got: &Evaluated, w: Want<'_>) -> [Case; 2] {
    let value = got.value();
    let err = |expected: f64| {
        value.map(|v| {
            if w.relative {
                (v / expected - 1.0).abs()
            } else {
                (v - expected).abs()
            }
        })
    };
    let within = |e: Option<f64>| e.is_some_and(|e| e <= w.tolerance);
    let moved = if w.relative {
        w.expected * (1.0 + 2.0 * w.tolerance)
    } else {
        w.expected + 2.0 * w.tolerance
    };
    let status = status_of(got);
    let shown = matches!(status.as_str(), "ok" | "wide");
    let case = |name: String, expected: f64, control: bool| {
        let e = err(expected);
        Case {
            name,
            quantity: w.quantity.into(),
            reference: w.reference.clone(),
            expected: Some(expected),
            expected_refusal: None,
            value,
            status: status.clone(),
            tolerance: w.tolerance,
            relative: w.relative,
            unit: w.unit.into(),
            error: e,
            control,
            holds: if control {
                !within(e)
            } else {
                shown && within(e)
            },
        }
    };
    [
        case(w.name.clone(), w.expected, false),
        case(
            format!(
                "control: {} against a reference moved by twice the tolerance",
                w.name
            ),
            moved,
            true,
        ),
    ]
}

fn refusal_case(got: &Evaluated, name: String, quantity: &str, want: &str) -> Case {
    let status = status_of(got);
    Case {
        name,
        quantity: quantity.into(),
        reference: format!("refused, {want}"),
        expected: None,
        expected_refusal: Some(want.into()),
        value: got.value(),
        holds: got.value().is_none() && status == want,
        status,
        tolerance: 0.0,
        relative: false,
        unit: String::new(),
        error: None,
        control: false,
    }
}

/// `−60` over the least-squares slope of `level(t)` on `[a, b]`, by Simpson's rule.
fn fitted_t(level: impl Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    let n = 20_000;
    let h = (b - a) / n as f64;
    let simpson = |f: &dyn Fn(f64) -> f64| {
        let mut s = f(a) + f(b);
        for i in 1..n {
            let t = a + i as f64 * h;
            s += if i % 2 == 1 { 4.0 } else { 2.0 } * f(t);
        }
        s * h / 3.0
    };
    let len = b - a;
    let tm = simpson(&|t| t) / len;
    let lm = simpson(&|t| level(t)) / len;
    let stl = simpson(&|t| (t - tm) * (level(t) - lm));
    let stt = simpson(&|t| (t - tm) * (t - tm));
    -60.0 / (stl / stt)
}

/// The time at which a falling `level(t)` reaches `l`, by bisection on `[0, hi]`.
fn crossing(level: impl Fn(f64) -> f64, l: f64, hi: f64) -> f64 {
    let (mut a, mut b) = (0.0, hi);
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        if level(m) > l {
            a = m;
        } else {
            b = m;
        }
    }
    0.5 * (a + b)
}

/// R15: T15 and the decay ranges a user chooses (upstream's TR list: each range from −5 dB down
/// its span), read by T20's and T30's regression.
pub fn r15() -> BedRun {
    let mut cases = Vec::new();
    let spans = [10u32, 25, 40, 50];
    let custom: Vec<Extra> = spans.iter().map(|&s| Extra::Decay { span_db: s }).collect();
    let name_of = |span: u32| {
        if span == 15 {
            "t15_s".to_string()
        } else {
            format!("t{span}_s")
        }
    };
    // Exponential decays: every range gives T exactly.
    for (t60, dt) in [(0.8, 0.001), (1.5, 0.001), (2.7, 0.01)] {
        let n = (2.5 * t60 / dt) as usize; // to −150 dB
        let s = EnergySeries::complete(dt, exp_bins(1.0, tau(t60), dt, n)).expect("a series");
        for span in std::iter::once(15).chain(spans) {
            let q = name_of(span);
            let got = read(&s, Arrival::at(0.0), &custom, &q);
            cases.extend(value_cases(
                &got,
                Want {
                    name: format!(
                        "exponential T = {t60} s, {} ms bins: T over -5 to -{} dB",
                        dt * 1000.0,
                        5 + span
                    ),
                    quantity: &q,
                    reference: format!(
                        "T = {t60} s: the Schroeder curve of e^(-t/tau) is a line of -60/T dB/s"
                    ),
                    expected: t60,
                    tolerance: 0.005,
                    relative: true,
                    unit: "relative",
                },
            ));
        }
    }
    // A two-slope decay: each range reads its own part of the curve.
    let (a, t1, b, t2, dt) = (1.0, 0.6, 0.002, 2.4, 0.001);
    let (tau1, tau2) = (tau(t1), tau(t2));
    let n = (2.5 * t2 / dt) as usize;
    let bins: Vec<f64> = exp_bins(a, tau1, dt, n)
        .iter()
        .zip(exp_bins(b, tau2, dt, n))
        .map(|(x, y)| x + y)
        .collect();
    let s = EnergySeries::complete(dt, bins).expect("a series");
    let schroeder = |t: f64| a * tau1 * (-t / tau1).exp() + b * tau2 * (-t / tau2).exp();
    // The series ends at n·dt: its Schroeder integral is the continuous one less what follows.
    let end = n as f64 * dt;
    let level = |t: f64| {
        10.0 * ((schroeder(t) - schroeder(end)) / (schroeder(0.0) - schroeder(end))).log10()
    };
    for span in std::iter::once(15).chain(spans) {
        let q = name_of(span);
        let lo = -5.0 - f64::from(span);
        let (ta, tb) = (crossing(level, -5.0, end), crossing(level, lo, end));
        let want = fitted_t(level, ta, tb);
        let got = read(&s, Arrival::at(0.0), &custom, &q);
        cases.extend(value_cases(
            &got,
            Want {
                name: format!("two slopes (T {t1} s, then {t2} s at {} dB): T over -5 to {lo} dB", 10.0 * (b * tau2 / (a * tau1)).log10().round()),
                quantity: &q,
                reference: format!("-60 over the least-squares slope of the closed-form Schroeder level between its -5 and {lo} dB crossings ({ta:.4} s, {tb:.4} s)"),
                expected: want,
                tolerance: 0.005,
                relative: true,
                unit: "relative",
            },
        ));
    }
    // The same code as T20 and T30: a span of 20 and of 30 give them exactly.
    let both = [Extra::Decay { span_db: 20 }, Extra::Decay { span_db: 30 }];
    for (span, fixed) in [(20, "t20_s"), (30, "t30_s")] {
        let got = read(&s, Arrival::at(0.0), &both, &format!("t{span}_s"));
        let want = read(&s, Arrival::at(0.0), &both, fixed)
            .value()
            .unwrap_or(f64::NAN);
        cases.extend(value_cases(
            &got,
            Want {
                name: format!("two slopes: a chosen span of {span} dB against {fixed} itself"),
                quantity: &format!("t{span}_s"),
                reference: format!("{fixed} of the same series: one regression, one code path"),
                expected: want,
                tolerance: 1e-12,
                relative: true,
                unit: "relative",
            },
        ));
    }
    // A decay that stops 50 dB down: a range below it is refused, not extrapolated.
    let t60 = 1.0;
    let cut = (50.0 / 60.0 * t60 / 0.001) as usize;
    let s = EnergySeries::new(0.001, exp_bins(1.0, tau(t60), 0.001, cut)).expect("a series");
    for span in [15u32, 50] {
        let q = name_of(span);
        let got = read(&s, Arrival::at(0.0), &custom, &q);
        if span == 15 {
            cases.extend(value_cases(
                &got,
                Want {
                    name: "exponential T = 1 s cut 50 dB down: T15, inside what the series holds"
                        .into(),
                    quantity: &q,
                    reference: "T = 1 s".into(),
                    expected: 1.0,
                    tolerance: 0.005,
                    relative: true,
                    unit: "relative",
                },
            ));
        } else {
            cases.push(refusal_case(
                &got,
                "exponential T = 1 s cut 50 dB down: T over -5 to -55 dB".into(),
                &q,
                "range_not_reached",
            ));
        }
    }
    bed(
        "m12c-r15",
        "R15: T15 (-5 to -20 dB) and decay ranges chosen from -5 dB down 10 to 60 dB, read by T20's and T30's regression through the report's own path (results::report::parameters_with)",
        cases,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r15_passes_and_its_controls_are_caught() {
        let b = r15();
        for c in &b.cases {
            assert!(c.holds, "{c:?}");
        }
        assert!(b.pass);
        assert!(b.cases.iter().filter(|c| c.control).count() >= 10);
    }

    /// `a` and `b` alike: the same strings, booleans and shape, numbers within `1e-9` relative
    /// (a debug and a release build round the last bits differently).
    fn alike(a: &serde_json::Value, b: &serde_json::Value, at: &str) {
        use serde_json::Value as V;
        match (a, b) {
            (V::Number(x), V::Number(y)) => {
                let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
                let scale = x.abs().max(y.abs()).max(1e-300);
                assert!((x - y).abs() <= 1e-9 * scale, "{at}: {x} vs {y}");
            }
            (V::Array(x), V::Array(y)) => {
                assert_eq!(x.len(), y.len(), "{at}");
                for (i, (p, q)) in x.iter().zip(y).enumerate() {
                    alike(p, q, &format!("{at}.{i}"));
                }
            }
            (V::Object(x), V::Object(y)) => {
                let (kx, ky): (Vec<_>, Vec<_>) = (x.keys().collect(), y.keys().collect());
                assert_eq!(kx, ky, "{at}");
                for (k, p) in x {
                    alike(p, &y[k], &format!("{at}.{k}"));
                }
            }
            _ => assert_eq!(a, b, "{at}"),
        }
    }

    /// The committed artifact `tools/bed/summary.py` reads is what this build computes, case for
    /// case (`simpa bed-extra <bed> --out beds/m12c-<bed>.json` writes it).
    pub(crate) fn committed_is_fresh(name: &str, fresh: &BedRun) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../beds/m12c-{name}.json"));
        let committed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let fresh = serde_json::to_value(fresh).unwrap();
        for k in ["bed", "pass", "failures", "worst", "cases"] {
            alike(&committed[k], &fresh[k], k);
        }
    }

    #[test]
    fn the_committed_r15_artifact_is_this_build_s() {
        committed_is_fresh("r15", &r15());
    }

    #[test]
    fn the_two_slope_reference_differs_by_range() {
        // Says no: were the ranges read alike, T15 and T55 of the two-slope decay would agree.
        let b = r15();
        let get = |q: &str| {
            b.cases
                .iter()
                .find(|c| c.quantity == q && c.name.starts_with("two slopes (") && !c.control)
                .and_then(|c| c.expected)
                .unwrap()
        };
        assert!(
            get("t50_s") / get("t15_s") > 1.2,
            "{} {}",
            get("t15_s"),
            get("t50_s")
        );
    }
}
