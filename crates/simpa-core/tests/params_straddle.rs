//! The bin straddling a window edge `te` (Theorem-CD, `docs/investigations/2026-09-27-edt-simplify/
//! FINAL.md` section 2; `docs/params.md`, "The bin straddling te"). Inside a bin the histogram does
//! not say where the energy arrived, so C and D are known only to within that bin wholly early or
//! wholly late. The bed's set A (`docs/investigations/2026-10-02-bed/`) found C50 and D50 answered
//! with no mark up to 0.50 dB and 0.054 off at a step of 10 ms, the te-straddling bin holding a
//! strong reflection the curve's in-bin decay split the wrong way (`s2|B26|rec1|R0.1|8000Hz|10ms`).

use simpa_core::params::EnergySeries;
use simpa_core::params::decay::{self, Arrival, limits};

/// A direct sound `direct` at `ta`, an exponential reverberation of total 1 from `ta` at T60
/// `t60`, and one reflection `refl` at `t_r`: its exact C50, C80 and D50, and its histogram at
/// step `dt` over `len_s`.
struct Echo {
    ta: f64,
    t60: f64,
    direct: f64,
    t_r: f64,
    refl: f64,
}

impl Echo {
    fn k(&self) -> f64 {
        6.0 * std::f64::consts::LN_10 / self.t60
    }

    /// Energy arriving in `[a, b)`.
    fn energy(&self, a: f64, b: f64) -> f64 {
        let k = self.k();
        let rev = |t: f64| 1.0 - (-k * (t - self.ta).max(0.0)).exp();
        let mut e = rev(b) - rev(a);
        if self.ta >= a && self.ta < b {
            e += self.direct;
        }
        if self.t_r >= a && self.t_r < b {
            e += self.refl;
        }
        e
    }

    fn bins(&self, dt: f64, len_s: f64) -> Vec<f64> {
        let n = (len_s / dt).round() as usize;
        (0..n)
            .map(|i| self.energy(i as f64 * dt, (i + 1) as f64 * dt))
            .collect()
    }

    fn total(&self) -> f64 {
        self.direct + 1.0 + self.refl
    }

    /// `(early, late)` for a window `te` from the arrival.
    fn split(&self, te: f64) -> (f64, f64) {
        let early = self.energy(0.0, self.ta + te);
        (early, self.total() - early)
    }

    fn clarity_db(&self, te: f64) -> f64 {
        let (e, l) = self.split(te);
        10.0 * (e / l).log10()
    }

    fn definition(&self, te: f64) -> f64 {
        let (e, _) = self.split(te);
        e / self.total()
    }
}

/// The bed's B26 case in miniature: `ta + 50 ms` falls 11 % into a 10 ms bin, and a strong
/// reflection arrives later in that same bin.
fn b26_like() -> Echo {
    Echo {
        ta: 0.0311,
        t60: 1.0,
        direct: 0.3,
        t_r: 0.0870,
        refl: 0.3,
    }
}

#[test]
fn a_reflection_in_the_bin_straddling_te_is_not_answered_without_its_bracket() {
    let e = b26_like();
    let dt = 0.01;
    let s = EnergySeries::complete(dt, e.bins(dt, 3.0)).unwrap();
    let p = decay::evaluate(&s, Arrival::at(e.ta));
    let truth = e.clarity_db(0.05);
    let c50 = *p.c50_db.as_ref().unwrap();
    // The in-bin decay splits the bin's energy the wrong way: off by more than the limit.
    assert!((c50 - truth).abs() > limits::CLARITY_DB, "{c50} vs {truth}");
    // So the value carries the bracket, beyond the limit, and the truth is inside it.
    let st = p.c50_straddle.expect("c50's bracket");
    assert!(st.beyond_limit, "{st:?}");
    assert!(st.lo <= truth && truth <= st.hi, "{truth} outside {st:?}");
    assert!(st.lo <= c50 && c50 <= st.hi, "{c50} outside {st:?}");
    // D50 the same.
    let truth = e.definition(0.05);
    let d50 = *p.d50.as_ref().unwrap();
    assert!((d50 - truth).abs() > limits::DEFINITION, "{d50} vs {truth}");
    let st = p.d50_straddle.expect("d50's bracket");
    assert!(
        st.beyond_limit && st.lo <= truth && truth <= st.hi,
        "{truth} {st:?}"
    );
}

#[test]
fn the_bracket_is_the_bin_wholly_early_and_wholly_late() {
    let e = b26_like();
    let dt = 0.01;
    let bins = e.bins(dt, 3.0);
    let s = EnergySeries::complete(dt, bins.clone()).unwrap();
    let p = decay::evaluate(&s, Arrival::at(e.ta));
    // The window edge falls in bin 8, [80, 90) ms: everything up to bin 8, then bin 8 too.
    let total: f64 = bins.iter().sum();
    let before: f64 = bins[..8].iter().sum();
    let with: f64 = before + bins[8];
    let c = |early: f64| 10.0 * (early / (total - early)).log10();
    let st = p.c50_straddle.unwrap();
    assert!((st.lo - c(before)).abs() < 1e-9, "{st:?} {}", c(before));
    assert!((st.hi - c(with)).abs() < 1e-9, "{st:?} {}", c(with));
    let st = p.d50_straddle.unwrap();
    assert!((st.lo - before / total).abs() < 1e-12 && (st.hi - with / total).abs() < 1e-12);
}

#[test]
fn a_smooth_decay_at_a_fine_step_keeps_its_value_within_the_limit() {
    // Says no to a bracket that marks everything: no reflection, 1 ms, T60 2 s. The bin holds
    // 0.35 % of the late energy, 0.015 dB of C.
    let e = Echo {
        ta: 0.0311,
        t60: 2.0,
        direct: 0.3,
        t_r: 10.0,
        refl: 0.0,
    };
    let dt = 0.001;
    let s = EnergySeries::complete(dt, e.bins(dt, 5.0)).unwrap();
    let p = decay::evaluate(&s, Arrival::at(e.ta));
    for (te, v, st) in [
        (0.05, &p.c50_db, p.c50_straddle),
        (0.08, &p.c80_db, p.c80_straddle),
    ] {
        let v = *v.as_ref().unwrap();
        assert!((v - e.clarity_db(te)).abs() < 0.01, "{v}");
        let st = st.unwrap();
        assert!(!st.beyond_limit, "{st:?}");
        assert!(st.lo <= v && v <= st.hi);
    }
    assert!(!p.d50_straddle.unwrap().beyond_limit);
    // A window edge on a bin edge has no straddling bin: the bracket is the value.
    let e2 = Echo { ta: 0.03, ..e };
    let s = EnergySeries::complete(dt, e2.bins(dt, 5.0)).unwrap();
    let p = decay::evaluate(&s, Arrival::at(e2.ta));
    let st = p.c50_straddle.unwrap();
    let v = *p.c50_db.as_ref().unwrap();
    assert!(
        (st.hi - st.lo).abs() < 1e-9 && (st.lo - v).abs() < 1e-9,
        "{st:?} {v}"
    );
}

#[test]
fn a_refused_value_has_no_bracket() {
    let e = b26_like();
    let dt = 0.01;
    // Ends before the window edge.
    let s = EnergySeries::complete(dt, e.bins(dt, 0.07)).unwrap();
    let p = decay::evaluate(&s, Arrival::at(e.ta));
    assert!(p.c50_db.is_err() && p.c50_straddle.is_none());
}
