//! Tutorial 1 against upstream's GUI (M7 follow-ups; the M7 critic). On the same 2019 `.recp`,
//! upstream's stored `Acoustic parameters.gabe` gives C80, D50 and Ts that differ from ours, and
//! the differences had been put down to upstream's method without anything showing that the
//! method explains all of them.
//!
//! Here upstream's algorithm (`src/isimpa/data_manager/projet_calculation.cpp` at `929a5c8`,
//! `OnMenuDoAcousticParametersComputation` and the functions it calls) is ported line by line, in
//! `f32` as upstream computes, and run on the `.recp` stored in `tutorial_1.proj`:
//! 1. **With the time-zero threshold of the GUI that wrote the table it reproduces the table**:
//!    all 216 values of Receiver 1 (the only receiver with one), NaN where it holds NaN, bit for
//!    bit or, where the GUI's C runtime rounded `log10f` differently, within 2 units in the last
//!    place (6·10⁻⁶ relative for a decay time, whose regression carries it). That threshold is
//!    `EPSILON`, 10⁻⁶ Pa², before upstream's commit `f50c36febd` (2020-12-04) set it to 10⁻¹⁸
//!    (`git show f50c36febd -- src/isimpa/data_manager/projet_calculation.cpp`; the file at
//!    `e9da8b3f12`, the last change before, is the 2019 GUI's, and differs from `929a5c8`'s in that
//!    threshold and in translation macros only).
//! 2. **Then the method is changed one step at a time towards ours**, and each step's effect is
//!    measured: the 2019 threshold to the pinned one; the boundary bin C counts twice, counted
//!    once; time zero moved from the label rule (`GetTimeDecay`) to the direct sound's arrival
//!    `r/c`, the window edge splitting its bin in proportion; and, last, our curve
//!    (`params::decay`), which the test holds within what moving the edge's bin wholly early or
//!    wholly late can change.
//!
//! Nothing here runs a solver.

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;

use simpa_core::formats::gabe::{self, Gabe};
use simpa_core::geometry::import::zip::Archive;
use simpa_core::params::decay::{self, Arrival};
use simpa_core::params::{EnergySeries, ParamError};

const PROJ: &str = "src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj";
const RUN: &str = "instance2/report/SPPS/2019-06-07_11h58m41s/";
/// The receiver of the 2019 run whose parameters upstream's GUI stored (Receiver 2's were never
/// computed: its folder holds no `Acoustic parameters.gabe`), its position, and the source's
/// (`config.xml` of the run).
const RECEIVERS: [(&str, [f64; 3]); 1] = [("Receiver 1", [1.0, 1.0, 1.8])];
const SOURCE: [f64; 3] = [3.0, 5.0, 1.8];

fn read(z: &Archive, name: &str) -> Gabe {
    gabe::read(&z.read(&format!("{RUN}{name}")).unwrap()).unwrap()
}

// --- upstream's algorithm, ported ----------------------------------------------------------------

/// `p_0 = 1/pow((float)(20*pow(10.f,(int)-6)),(int)2)` (line 41): the `pow`s of a `float` and an
/// `int` are `double`s in C++11, the result stored as a `float`.
fn p_0() -> f32 {
    let x = 20.0f32 * 10f32.powi(-6);
    (1.0 / f64::from(x).powi(2)) as f32
}

/// `to_deciBelP0`: `10*log10f(wjVal*p_0)`.
fn db_p0(w: f32) -> f32 {
    10.0 * (w * p_0()).log10()
}

/// `Convertor::ToFloat(label.Left(label.find(" ")))` (`projet_calculation.cpp:854`,
/// `sppsString.cpp:43-60`): parsed as a `double`, returned as `float_t`.
fn label_time_ms(label: &[u8]) -> f32 {
    let s = String::from_utf8_lossy(label);
    let head = s.split(' ').next().unwrap();
    head.parse::<f64>().unwrap() as f32
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Y,
    XY,
    X,
    X2,
}

/// `GetSumLimit` (lines 95-117): over the steps whose label lies in `[from, to]` (`to == -1`:
/// no end), breaking at the first label past `to`.
fn sum_limit(from: f32, to: f32, time: &[f32], row: &[f32], op: Op, n: &mut i32) -> f32 {
    let mut s = 0.0f32;
    for (k, &t) in time.iter().enumerate() {
        if t >= from && (t <= to || to == -1.0) {
            *n += 1;
            s += match op {
                Op::Y => row[k],
                Op::XY => row[k] * t,
                Op::X => t,
                Op::X2 => t * t,
            };
        }
        if t > to && to != -1.0 {
            break;
        }
    }
    s
}

fn sum(from: f32, to: f32, time: &[f32], row: &[f32], op: Op) -> f32 {
    sum_limit(from, to, time, row, op, &mut 0)
}

/// The threshold `GetTimeDecay` is called with. At the pinned commit, `refValue = pow(10,
/// -180.0f/10.0f)` stored as a `float` (lines 321, 357, 393, 424). Before upstream's commit
/// `f50c36febd` (2020-12-04, "about issue #7 set epsilon value as low as possible in order to not
/// skip first sound wave"), `EPSILON`, `(decimal)0.000001` (`lib_interface/Core/mathlib.h:56`,
/// `decimal` being `float`): what the GUI that wrote the 2019 table used.
#[derive(Clone, Copy, Debug)]
enum Threshold {
    Pinned,
    Before2020,
}

impl Threshold {
    fn value(self) -> f32 {
        match self {
            Threshold::Pinned => 10f64.powf(f64::from(-180.0f32 / 10.0f32)) as f32,
            Threshold::Before2020 => 0.000001f32,
        }
    }
}

/// `GetTimeDecay` (lines 127-139): the label of the last step before the energy first changes
/// from the first step's by at least the threshold, in Pa², an absolute level.
fn time_decay(time: &[f32], row: &[f32], th: Threshold) -> f32 {
    let from = th.value();
    let mut t0 = time[0];
    let last = row[0];
    for (k, &e) in row.iter().enumerate() {
        if (e - last).abs() >= from {
            break;
        }
        t0 = time[k];
    }
    t0
}

/// `double wt0 = GetTimeDecay(..); double wte = wt0 + te;`, passed on as `wxFloat32`.
fn window_end(t0: f32, te: f32) -> f32 {
    (f64::from(t0) + f64::from(te)) as f32
}

/// `Compute_C_Param` (lines 351-379): `10·log10f(Σ[t0, t0+te] / Σ[t0+te, ∞))`, both on labels,
/// both ends included.
fn c_upstream(te: f32, time: &[f32], row: &[f32], th: Threshold) -> f32 {
    let t0 = time_decay(time, row, th);
    let te = window_end(t0, te);
    10.0 * (sum(t0, te, time, row, Op::Y) / sum(te, -1.0, time, row, Op::Y)).log10()
}

/// `Compute_D_Param` (lines 385-410).
fn d_upstream(te: f32, time: &[f32], row: &[f32], th: Threshold) -> f32 {
    let t0 = time_decay(time, row, th);
    let te = window_end(t0, te);
    (sum(t0, te, time, row, Op::Y) / sum(t0, -1.0, time, row, Op::Y)) * 100.0
}

/// `Compute_TS_Param` (lines 416-441): `Σ E·t / Σ E` from `t0`, `t` the absolute label, ms.
fn ts_upstream(time: &[f32], row: &[f32], th: Threshold) -> f32 {
    let t0 = time_decay(time, row, th);
    sum(t0, -1.0, time, row, Op::XY) / sum(t0, -1.0, time, row, Op::Y)
}

/// `MakeSchroederArray` (lines 68-83) of one row.
fn schroeder(row: &[f32]) -> Vec<f32> {
    let mut out = row.to_vec();
    let mut w = 0.0f32;
    for k in (0..row.len()).rev() {
        w += row[k];
        out[k] = db_p0(w);
    }
    out
}

/// `GetTimeRange` (lines 143-170) then `ComputeLinearRegression` (lines 175-186), and
/// `(-60/a)/1000` (line 205): a decay time in s.
fn tr_upstream(from_db: f32, to_db: f32, time: &[f32], row_db: &[f32]) -> f32 {
    let (mut deb, mut end) = (time[0], time[time.len() - 1]);
    let last = row_db[0];
    let mut inside = false;
    for (k, &l) in row_db.iter().enumerate() {
        if !inside {
            if (l - last).abs() >= from_db {
                deb = time[k];
                inside = true;
            }
        } else if (l - last).abs() >= to_db {
            end = time[k];
            break;
        }
    }
    let mut n = 0;
    let sx = sum_limit(deb, end, time, row_db, Op::X, &mut n);
    let sy = sum(deb, end, time, row_db, Op::Y);
    let sx2 = sum(deb, end, time, row_db, Op::X2);
    let sxy = sum(deb, end, time, row_db, Op::XY);
    let n = n as f32;
    let a = (n * sxy - sx * sy) / (n * sx2 - sx * sx);
    (-60.0 / a) / 1000.0
}

// --- the steps from upstream's method to ours ---------------------------------------------------

/// The energy of `row` (bin `k` over `[k·dt, (k+1)·dt)`) between `a` and `b` s, a bin cut by an
/// edge counted in proportion to its overlap.
fn energy_between(row: &[f64], dt: f64, a: f64, b: f64) -> f64 {
    row.iter()
        .enumerate()
        .map(|(k, &e)| {
            let (lo, hi) = (k as f64 * dt, (k + 1) as f64 * dt);
            let overlap = (hi.min(b) - lo.max(a)).max(0.0);
            e * overlap / dt
        })
        .sum()
}

/// A value, or the value a refusal carries with it (`params::decay` reports none as the
/// quantity; here it is the thing compared).
fn value_of(r: &Result<f64, ParamError>) -> f64 {
    match r {
        Ok(v) => *v,
        Err(e) => {
            let v = serde_json::to_value(e).unwrap();
            v["why"]["value"]
                .as_f64()
                .unwrap_or_else(|| panic!("no value in {e}"))
        }
    }
}

#[test]
fn upstreams_parameters_are_reproduced_on_the_2019_recp_and_the_gap_to_ours_is_the_method() {
    let bytes = std::fs::read(paths::upstream_file(PROJ)).unwrap();
    let z = Archive::parse(&bytes).unwrap();
    let dt_s = f64::from(0.01f32);
    let c = f64::from(simpa_core::results::spps::solver_speed_of_sound(20.0));
    let radius = f64::from(0.31f32);
    let (mut reproduced, mut nan, mut rounded, mut worst_time) = (0usize, 0usize, 0usize, 0.0f64);
    let mut worst_ulps = 0i64;
    let (mut curve_values, mut curve_diffs, mut worst_curve) = (0usize, 0usize, 0.0f64);
    let mut steps: Vec<[f64; 14]> = Vec::new();
    for (label, at) in RECEIVERS {
        let stored = read(
            &z,
            &format!("Punctual receivers/{label}/Acoustic parameters.gabe"),
        );
        let recp = read(&z, &format!("Punctual receivers/{label}/Sound level.recp"));
        let curves = read(
            &z,
            &format!("Punctual receivers/{label}/Schroeder curves.gabe"),
        );
        let time: Vec<f32> = recp.columns[0]
            .strings()
            .unwrap()
            .iter()
            .map(|l| label_time_ms(l))
            .collect();
        let bands: Vec<(String, Vec<f32>)> = recp.columns[1..]
            .iter()
            .map(|c| {
                (
                    String::from_utf8_lossy(c.name()).into_owned(),
                    c.floats().unwrap().to_vec(),
                )
            })
            .collect();
        let r = ((at[0] - SOURCE[0]).powi(2) + (at[1] - SOURCE[1]).powi(2)).sqrt();
        let t_a = r / c;
        println!(
            "\n{label}: r {r:.4} m, r/c {:.3} ms; labels {} to {} ms",
            1e3 * t_a,
            time[0],
            time[time.len() - 1]
        );
        println!(
            "{:>9} | {:^13} | {:^34} | {:^27} | {:^20}",
            "band",
            "t0 ms: U P",
            "C80 dB: U | P P1 P2 O",
            "D50 %: U | P P2 O",
            "Ts ms: U | P-r/c O"
        );
        for (name, row) in &bands {
            let cell = |col: &str| -> f32 {
                let i = stored.row_index(name.as_bytes()).unwrap();
                stored.column(col.as_bytes()).unwrap().floats().unwrap()[i]
            };
            // 1. The method of the GUI that wrote the table, ported, reproduces it: every value
            //    bit for bit, NaN where it holds NaN.
            let db = schroeder(row);
            let band_row = curves.row_index(name.as_bytes()).unwrap();
            for (k, &l) in db.iter().enumerate() {
                let stored_db = curves.columns[k + 1].floats().unwrap()[band_row];
                let d = f64::from(l) - f64::from(stored_db);
                if l.to_bits() != stored_db.to_bits() {
                    curve_diffs += 1;
                    worst_curve = worst_curve.max(d.abs());
                }
                curve_values += 1;
            }
            let old = Threshold::Before2020;
            for (col, p) in [
                ("Sound level (dB)", db_p0(sum(0.0, -1.0, &time, row, Op::Y))),
                ("C-50 (dB)", c_upstream(50.0, &time, row, old)),
                ("C-80 (dB)", c_upstream(80.0, &time, row, old)),
                ("D-50 (%)", d_upstream(50.0, &time, row, old)),
                ("Ts (ms)", ts_upstream(&time, row, old)),
                ("RT-15 (s)", tr_upstream(5.0, 20.0, &time, &db)),
                ("RT-30 (s)", tr_upstream(5.0, 35.0, &time, &db)),
                ("EDT (s)", tr_upstream(0.0, 10.0, &time, &db)),
            ] {
                let u = cell(col);
                let exact = u.to_bits() == p.to_bits() || (u.is_nan() && p.is_nan());
                if exact {
                    reproduced += 1;
                } else {
                    // The GUI's `log10f` (its C runtime's, not Rust's) can differ in the last bit,
                    // and a decay time's regression carries that through its f32 sums. Nothing
                    // more is allowed: a few units in the last place for a level or a ratio, and
                    // 1e-5 relative for a decay time.
                    assert!(
                        u.is_finite() && p.is_finite(),
                        "{label} {name} {col}: {u} {p}"
                    );
                    let ulps =
                        (i64::from(u.to_bits() as i32) - i64::from(p.to_bits() as i32)).abs();
                    let rel = f64::from((u / p - 1.0).abs());
                    if col.ends_with("(s)") {
                        worst_time = worst_time.max(rel);
                        assert!(rel < 1e-5, "{label} {name} {col}: stored {u}, port {p}");
                    } else {
                        worst_ulps = worst_ulps.max(ulps);
                        assert!(ulps <= 8, "{label} {name} {col}: stored {u}, port {p}");
                    }
                    rounded += 1;
                }
                nan += usize::from(u.is_nan());
            }

            // 2. From the 2019 GUI's method to ours, one step at a time, in f64 on the same
            //    values.
            let now = Threshold::Pinned;
            let (t0_u, t0_p) = (time_decay(&time, row, old), time_decay(&time, row, now));
            let (c80_u, d50_u, ts_u) = (cell("C-80 (dB)"), cell("D-50 (%)"), cell("Ts (ms)"));
            // P: upstream's GUI at the pinned commit, time zero by the 1e-18 threshold.
            let (c80_p, d50_p, ts_p) = (
                c_upstream(80.0, &time, row, now),
                d_upstream(50.0, &time, row, now),
                ts_upstream(&time, row, now),
            );
            let e: Vec<f64> = row.iter().map(|&x| f64::from(x)).collect();
            let t0 = f64::from(t0_p) / 1e3;
            // P1: C with the boundary bin counted once: early the bins whose end label is at most
            // t0 + te, late the rest.
            let early_bins = |te: f64| -> f64 {
                e.iter()
                    .enumerate()
                    .filter(|(k, _)| {
                        let end = (*k + 1) as f64 * dt_s;
                        end >= t0 - 1e-9 && end <= t0 + te + 1e-9
                    })
                    .map(|(_, x)| x)
                    .sum()
            };
            let total_from_t0: f64 = e
                .iter()
                .enumerate()
                .filter(|(k, _)| (*k + 1) as f64 * dt_s >= t0 - 1e-9)
                .map(|(_, x)| x)
                .sum();
            let c80_p1 = 10.0 * (early_bins(0.08) / (total_from_t0 - early_bins(0.08))).log10();
            // P2: time zero at the direct sound's arrival r/c, everything from the first bin with
            // energy counted (the direct sound, spread from (r − R)/c, starts in it), the window's
            // edge splitting its bin in proportion.
            let all: f64 = e.iter().sum();
            let early = |te: f64| energy_between(&e, dt_s, 0.0, t_a + te);
            let c80_p2 = 10.0 * (early(0.08) / (all - early(0.08))).log10();
            let d50_p2 = 100.0 * early(0.05) / all;
            // O: ours, the series as `core::results` gives it to `params` (the arrival at r/c
            // with the direct sound spread over ±R/c, the early reverberation unresolved).
            let s = EnergySeries::new(dt_s, e.clone())
                .unwrap()
                .with_early_reverberation_unresolved();
            let p = decay::evaluate(&s, Arrival::spread(t_a, radius / c));
            let (c80_o, d50_o, ts_o) = (
                value_of(&p.c80_db),
                100.0 * value_of(&p.d50),
                1e3 * value_of(&p.ts_s),
            );
            // What remains between P2 and O is where inside the window edge's bin its energy
            // lies: that bin wholly early, or wholly late, brackets both.
            let bracket = |te: f64| {
                let edge = ((t_a + te) / dt_s).floor();
                (
                    energy_between(&e, dt_s, 0.0, edge * dt_s),
                    energy_between(&e, dt_s, 0.0, (edge + 1.0) * dt_s),
                )
            };
            let c = |x: f64| 10.0 * (x / (all - x)).log10();
            let (lo, hi) = bracket(0.08);
            for v in [c80_p2, c80_o] {
                assert!(
                    c(lo) - 1e-9 <= v && v <= c(hi) + 1e-9,
                    "{label} {name}: C80 {v} outside [{}, {}]",
                    c(lo),
                    c(hi)
                );
            }
            let (lo, hi) = bracket(0.05);
            for v in [d50_p2, d50_o] {
                assert!(
                    100.0 * lo / all - 1e-9 <= v && v <= 100.0 * hi / all + 1e-9,
                    "{label} {name}: D50 {v}"
                );
            }
            println!(
                "{name:>9} | {t0_u:>6.0} {t0_p:>6.0} | {c80_u:>6.2} | {c80_p:>6.2} {c80_p1:>6.2} \
                 {c80_p2:>6.2} {c80_o:>6.2} | {d50_u:>6.2} | {d50_p:>6.2} {d50_p2:>6.2} \
                 {d50_o:>6.2} | {ts_u:>6.1} | {:>6.1} {ts_o:>6.1}",
                f64::from(ts_p) - 1e3 * t_a
            );
            steps.push([
                f64::from(c80_u),
                f64::from(c80_p),
                c80_p1,
                c80_p2,
                c80_o,
                f64::from(d50_u),
                f64::from(d50_p),
                d50_p2,
                d50_o,
                f64::from(ts_u),
                f64::from(ts_p),
                f64::from(ts_p) - 1e3 * t_a,
                f64::from(ts_p) - 1e3 * (t_a + dt_s / 2.0),
                ts_o,
            ]);
            // Says no: the pinned GUI's time zero does not give the stored NaN, and C counted
            // with its boundary bin once does not give the stored C80.
            if c80_u.is_nan() {
                assert!(c80_p.is_finite(), "{name}");
            } else {
                assert!((f64::from(c80_u) - c80_p1).abs() > 0.5, "{name}");
            }
        }
    }
    println!(
        "\nSchroeder curves: {curve_diffs} of {curve_values} stored levels differ from the port, by \
         at most {worst_curve:.2e} dB"
    );
    println!(
        "{reproduced} stored values reproduced bit for bit ({nan} of them NaN), {rounded} to the \
         last bits: levels and ratios within {worst_ulps} units in the last place, decay times \
         within {worst_time:.1e} relative"
    );
    assert_eq!(reproduced + rounded, 27 * 8);
    // Receiver 2 has no stored table to compare with.
    assert!(
        z.read(&format!(
            "{RUN}Punctual receivers/Receiver 2/Acoustic parameters.gabe"
        ))
        .is_err()
    );
    // Each step's effect, over the receiver-bands where the stored value is a number.
    let names = [
        "C80: 2019 GUI -> pinned GUI (time zero's threshold 1e-6 -> 1e-18 Pa2)",
        "C80: -> the boundary bin counted once",
        "C80: -> time zero at r/c, the edge's bin split in proportion",
        "C80: -> ours (the curve inside a bin)",
        "D50: 2019 GUI -> pinned GUI",
        "D50: -> time zero at r/c, the edge's bin split in proportion",
        "D50: -> ours",
        "Ts: 2019 GUI -> pinned GUI",
        "Ts: -> less r/c (upstream weights by the absolute time)",
        "Ts: -> less half a bin (upstream's labels are the bins' ends)",
        "Ts: -> ours",
    ];
    let pairs = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (5, 6),
        (6, 7),
        (7, 8),
        (9, 10),
        (10, 11),
        (11, 12),
        (12, 13),
    ];
    for (what, (a, b)) in names.iter().zip(pairs) {
        let d: Vec<f64> = steps
            .iter()
            .filter(|s| s[0].is_finite())
            .map(|s| s[b] - s[a])
            .collect();
        let mean = d.iter().sum::<f64>() / d.len() as f64;
        let (lo, hi) = d
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| {
                (l.min(x), h.max(x))
            });
        println!(
            "{what}: mean {mean:+.3}, from {lo:+.3} to {hi:+.3} over {} receiver-bands",
            d.len()
        );
    }
}
