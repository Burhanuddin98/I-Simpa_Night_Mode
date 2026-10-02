//! EDT v2.1 for SPPS receiver histograms: a line-for-line port of
//! `docs/investigations/2026-09-27-edt-heldout/frozen2/method.py` (sha256 `029d90ac...`, Z = 2.5),
//! the method whose held-out verdict is `VERDICT-2.md`: Random-mode runs pass H1-H6; Energetic
//! mode fails H3, so its EDT is "not yet validated" ([`validated_for`]).
//!
//! **Do not tune this file.** It is the frozen method in Rust, shown equal to the Python on
//! round 2's inputs by `tests/edt_port.rs`. A change of method is a change of `frozen2`, a new
//! pre-registration and a new round, not an edit here. The names of the constants, the
//! comments and the order of the refusals follow the Python so the two can be read side by side.
//!
//! What it takes: `bins[k]`, the energy recorded in `[k·dt, (k+1)·dt)`; `t_arrival`, the time the
//! direct sound reaches the receiver's centre **including the source's emission delay, rounded up
//! to the next whole step** (`SppsResults::arrival_s`), or `None` (a celerity gradient);
//! `half_width`, `R/c` of the receiver ball. It returns the value and a range, `ok` when the range
//! is inside the 5 % JND and `wide` otherwise, or a refusal with a reason.

use schemars::JsonSchema;
use serde::Serialize;

/// The method and its version, as the results JSON names it.
pub const METHOD: &str = "edt_v2.1";

/// ISO 3382-1 Annex A. Status `ok` iff the shown half-width is inside it.
const JND: f64 = 0.05;
/// Burhan, 2026-10-02 00:24 (PREREG-2 amendment 1).
const Z: f64 = 2.5;
/// Schroeder samples between the ball's back and -10 dB.
const MIN_POINTS: usize = 8;
/// Refuse if the extrapolated unrecorded tail exceeds 2 % of S(t_-10dB).
const TAIL_SHARE: f64 = 0.02;
/// F1: refuse `receiver_too_large` when h / EDT exceeds this.
const K_BALL: f64 = 0.01;
/// F2: noise blocks span this many ball crossings (2h/dt bins each).
const BLOCK_CROSSINGS: f64 = 2.0;
/// F2: at least this many blocks in the noise window.
const MIN_BLOCKS: usize = 8;
/// F3: the w window runs from the first fitted sample to this fraction of the fit length past -10 dB.
const W_MARGIN: f64 = 0.75;
/// Never claim tighter than 0.5 %.
const HW_FLOOR: f64 = 0.005;
/// Speed of sound the default ball (R = 0.31 m) is converted with.
const C: f64 = 343.2;

/// Every reason the method refuses with, in the order frozen2 can reach them.
/// `not_decaying` has no input that reaches it (the Schroeder level is non-increasing and the
/// fit runs from above -10 dB to at or below it, so the slope is negative); frozen2 keeps the
/// branch as a guard and so does this port.
pub const REFUSAL_REASONS: [&str; 9] = [
    "no_energy",
    "no_energy_after_arrival",
    "run_too_short",
    "direct_only",
    "step_too_coarse",
    "not_decaying",
    "too_few_particles",
    "not_decaying_at_run_end",
    "receiver_too_large",
];

/// What the method says about the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The range is inside the 5 % JND.
    Ok,
    /// A value with a range wider than the JND: shown with its range (decision-log row 9).
    Wide,
    /// No value; `reason` says why.
    Refused,
}

/// frozen2's return value.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub edt: Option<f64>,
    pub edt_lo: Option<f64>,
    pub edt_hi: Option<f64>,
    pub status: Status,
    /// A refusal's code (one of [`REFUSAL_REASONS`]), or for `ok` and `wide` frozen2's own
    /// detail string `hw=…;fit=…;noise=…;tail=…;n=…`.
    pub reason: String,
}

impl Outcome {
    /// The refusal's code, when refused.
    pub fn refusal(&self) -> Option<&str> {
        (self.status == Status::Refused).then_some(self.reason.as_str())
    }
}

/// Whether EDT from a run of `computation_method` (0 random, 1 energetic) is validated:
/// Random passes VERDICT-2's H1-H6, Energetic fails H3 (`G4 far 1 ms`, `G4 far 2 ms`) and is
/// "not yet validated" until Burhan rules (VERDICT-2, ruling 2; default P11).
pub fn validated_for(computation_method: i32) -> bool {
    computation_method == 0
}

fn refuse(why: &str) -> Outcome {
    Outcome {
        edt: None,
        edt_lo: None,
        edt_hi: None,
        status: Status::Refused,
        reason: why.to_string(),
    }
}

/// Energy per hit `w` (the variance of a block sum is `w` times its mean) from the second
/// differences of `b`-bin block sums of `d`; `None` when fewer than 3 blocks fit or no block has
/// energy (`count_weight`).
fn count_weight(d: &[f64], b: usize) -> Option<f64> {
    let nb = d.len() / b;
    if nb < 3 {
        return None;
    }
    let c: Vec<f64> = (0..nb)
        .map(|j| d[j * b..(j + 1) * b].iter().sum())
        .collect();
    let m = c.len() / 2;
    let e1: f64 = c[..m].iter().sum();
    let e2: f64 = c[m..2 * m].iter().sum();
    let x = if e1 > e2 && e2 > 0.0 {
        (e1 / e2).ln() / m as f64
    } else {
        0.0
    };
    let mut q: Vec<f64> = Vec::new();
    for j in 1..c.len() - 1 {
        let r = c[j] - 0.5 * (c[j - 1] * (-x).exp() + c[j + 1] * x.exp());
        if c[j] > 0.0 {
            q.push(r * r / c[j]);
        }
    }
    if q.is_empty() {
        return None;
    }
    q.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let k = q.len();
    let median = if k % 2 == 1 {
        q[k / 2]
    } else {
        (q[k / 2 - 1] + q[k / 2]) / 2.0
    };
    Some(median / 0.4549 / (1.0 + 0.5 * x.cosh()))
}

/// Python's `'%.1e' % v` for a finite non-negative `v`.
fn sci1(v: f64) -> String {
    if v == 0.0 {
        return "0.0e+00".to_string();
    }
    let s = format!("{v:.1e}");
    let (mant, exp) = s.split_once('e').expect("an exponent");
    let e: i32 = exp.parse().expect("an integer exponent");
    format!("{mant}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// frozen2's `analyse`.
pub fn analyse(bins: &[f64], dt: f64, t_arrival: Option<f64>, half_width: Option<f64>) -> Outcome {
    let b: Vec<f64> = bins
        .iter()
        .map(|&x| if x < 0.0 { 0.0 } else { x })
        .collect();
    let n = b.len();
    if n < 4 || dt <= 0.0 || b.iter().sum::<f64>() <= 0.0 {
        return refuse("no_energy");
    }
    let h = half_width.unwrap_or(0.31 / C);
    // S[i] = energy after i*dt, exact at bin starts
    let mut s = vec![0.0; n];
    let mut acc = 0.0;
    for i in (0..n).rev() {
        acc += b[i];
        s[i] = acc;
    }

    // I1/I2: 0 dB at the FRONT of the ball's direct sound; first fitted sample after its BACK
    let mut k0: usize;
    let mut t_start: f64;
    match t_arrival {
        None => {
            k0 = b.iter().position(|&x| x > 0.0).expect("energy");
            t_start = (k0 + 1) as f64 * dt;
        }
        Some(ta) => {
            let f = ((ta - h) / dt).floor();
            k0 = if f > 0.0 { f as usize } else { 0 };
            t_start = ta + h;
            if k0 >= n || b[k0..].iter().sum::<f64>() <= 0.0 {
                return refuse("no_energy_after_arrival");
            }
            let k_on = k0 + b[k0..].iter().position(|&x| x > 0.0).expect("energy");
            if k_on as f64 * dt > t_start + dt {
                // blocked path: 0 dB at the first arrival
                k0 = k_on;
                t_start = (k_on + 1) as f64 * dt;
            }
        }
    }
    let m0 = n - k0;
    let level = |i: usize| 10.0 * (s[k0 + i].max(1e-300) / s[k0]).log10();
    let t_at = |i: usize| (k0 + i) as f64 * dt;
    let Some(i10) = (0..m0).find(|&i| level(i) <= -10.0) else {
        return refuse("run_too_short");
    };
    let sel: Vec<usize> = (0..=i10).filter(|&i| t_at(i) >= t_start - 1e-12).collect();
    if sel.len() < MIN_POINTS {
        // I3/I6: direct-only, or dt too coarse for it
        return refuse(if t_at(i10) <= t_start + dt {
            "direct_only"
        } else {
            "step_too_coarse"
        });
    }

    let t: Vec<f64> = sel.iter().map(|&i| t_at(i)).collect();
    let y: Vec<f64> = sel.iter().map(|&i| level(i)).collect();
    let np = t.len() as f64;
    let tb = t.iter().sum::<f64>() / np;
    let yb = y.iter().sum::<f64>() / np;
    let sxx: f64 = t.iter().map(|v| (v - tb) * (v - tb)).sum();
    let a: f64 = t
        .iter()
        .zip(&y)
        .map(|(ti, yi)| (ti - tb) * (yi - yb))
        .sum::<f64>()
        / sxx;
    if a >= 0.0 {
        return refuse("not_decaying");
    }
    let rss: f64 = t
        .iter()
        .zip(&y)
        .map(|(ti, yi)| {
            let r = yi - (yb + a * (ti - tb));
            r * r
        })
        .sum();
    let se = (rss / (t.len() as f64 - 2.0) / sxx).sqrt();

    // I4: unrecorded tail from the LAST recorded envelope (Hirata), never from the early slope
    let m = (n / 5).max(2); // last 20 % vs the 20 % before it
    let e1: f64 = b[n - 2 * m..n - m].iter().sum();
    let e2: f64 = b[n - m..].iter().sum();
    let u = if e2 == 0.0 {
        0.0
    } else if e1 > e2 {
        e2 * e2 / (e1 - e2)
    } else {
        f64::INFINITY
    };
    let s10 = s[k0 + i10];
    if s10 <= 0.0 {
        // nothing recorded after -10 dB: too few hits
        return refuse("too_few_particles");
    }
    if u > TAIL_SHARE * s10 {
        return refuse(if u.is_finite() {
            "run_too_short"
        } else {
            "not_decaying_at_run_end"
        });
    }

    // I5: counting noise from the bin scatter after -10 dB. Compound Poisson, hit-level w; the
    // slope variance is propagated exactly to first order below.
    let block = ((BLOCK_CROSSINGS * 2.0 * h / dt - 1e-9).ceil() as i64).max(1) as usize;
    // v2.1/F3: w is estimated where the fit lives, not from the far tail
    let i0 = k0 + sel[0];
    let j_end = n.min(
        (k0 + i10 + (W_MARGIN * sel.len() as f64).ceil() as usize).max(i0 + MIN_BLOCKS * block),
    );
    let w = count_weight(&b[i0..j_end], block);
    // first-order slope variance from hit noise: d slope / d b_j = g_j
    let mut g = Vec::with_capacity(n - i0);
    let mut run = 0.0;
    for (k, &i) in sel.iter().enumerate() {
        run += (t[k] - tb) / sxx / s[k0 + i];
        g.push(run);
    }
    let last = *g.last().expect("a fit");
    g.resize(n - i0, last);
    let sd = match w {
        None => 0.0,
        Some(w) => {
            let acc: f64 = b[i0..].iter().zip(&g).map(|(bj, gj)| bj * gj * gj).sum();
            4.343 * (w * acc).sqrt() / -a
        }
    };

    if h > K_BALL * (-60.0 / a) {
        // v2/F1: the unseen first h of the decay is a shelf the fit cannot see
        return refuse("receiver_too_large");
    }
    let rel = se / a;
    let q = (Z * (rel * rel + sd * sd).sqrt()).max(HW_FLOOR);
    let edt = -60.0 / a;
    let lo = edt / (1.0 + q);
    let hi = if q < 0.9 { edt / (1.0 - q) } else { 10.0 * edt };
    let hw = (hi - lo) / (2.0 * edt);
    let ok = hw <= JND && w.is_some();
    Outcome {
        edt: Some(edt),
        edt_lo: Some(lo),
        edt_hi: Some(hi),
        status: if ok { Status::Ok } else { Status::Wide },
        reason: format!(
            "hw={hw:.3};fit={:.3};noise={};tail={};n={}",
            se / -a,
            match w {
                None => "unknown".to_string(),
                Some(_) => format!("{sd:.3}"),
            },
            sci1(u / s10),
            t.len()
        ),
    }
}
