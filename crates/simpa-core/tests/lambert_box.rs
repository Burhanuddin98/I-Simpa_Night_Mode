//! A from-scratch Monte Carlo of M8's rooms, sharing no code with SPPS (M7 follow-ups, second
//! review; `docs/results.md`, "What M8 needs"). Evidence, not a gate: it is ignored and run on
//! purpose,
//!
//! `cargo test --release -p simpa-core --test lambert_box -- --ignored --nocapture`
//!
//! **Why.** SPPS's mean T30 lies above T_Eyring by 1 % at α 0.05 up to 9–11 % at α 0.4, the same in
//! both computation methods. Those share SPPS's whole transport, so their agreement cannot tell a
//! reference that does not fit the room from a solver defect. This is a second transport, written
//! here: a shoebox with every surface of absorption α and Lambert (cosine) reflection, rays from an
//! omnidirectional source at `c` = 343.2 m/s, each ray's energy multiplied by `1 − α` at every
//! reflection, and receiver balls of radius 0.31 m that collect a ray's energy times the length of
//! its path inside them, per time bin, as SPPS's receivers do. Nothing of SPPS is used: no mesh, no
//! time stepping, no random generator of its.
//!
//! It gives, per cell:
//! - the mean free path between reflections, against `4V/S`, and its relative variance `γ²`;
//! - the decay rate of the room's energy, fitted apart from `params`, against Eyring and against
//!   the rate the free-path distribution gives exactly for a renewal process (`E[(1 − α)·e^{λℓ/c}] =
//!   1`, which Kuttruff's `1 + (γ²/2)·ln(1 − α)` correction approximates);
//! - T30 and EDT at the receivers, from the histograms through `params` as SPPS's are read, at a step
//!   of 10 ms and 0.2 ms.

use std::f64::consts::{LN_10, PI};

use simpa_core::params::EnergySeries;
use simpa_core::params::decay::{Arrival, evaluate};

const C: f64 = 343.2;
const RADIUS: f64 = 0.31;

/// SplitMix64.
struct Rng(u64);

impl Rng {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// One of M8's rooms, with its source and three receivers as `m8_evidence.rs` sets them.
#[derive(Clone, Copy)]
struct Room {
    name: &'static str,
    size: [f64; 3],
    source: [f64; 3],
    receivers: [[f64; 3]; 3],
}

const ROOMS: [Room; 2] = [
    Room {
        name: "6x10x3",
        size: [6.0, 10.0, 3.0],
        source: [3.0, 5.0, 1.8],
        receivers: [[1.0, 1.0, 1.8], [3.0, 7.0, 1.8], [5.0, 8.5, 1.2]],
    },
    Room {
        name: "5x4x3",
        size: [5.0, 4.0, 3.0],
        source: [2.52, 1.97, 1.53],
        receivers: [[1.0, 1.0, 1.0], [4.0, 3.0, 2.0], [1.0, 3.0, 1.9]],
    },
];

impl Room {
    fn volume_area(&self) -> (f64, f64) {
        let [a, b, c] = self.size;
        (a * b * c, 2.0 * (a * b + b * c + a * c))
    }
}

/// What one batch of rays leaves: the room's energy and each receiver's, per bin of `dt`, and the
/// free paths between reflections.
struct Tally {
    dt: f64,
    room: Vec<f64>,
    receivers: Vec<Vec<f64>>,
    paths: (f64, f64, f64),
}

impl Tally {
    fn new(dt: f64, bins: usize, receivers: usize) -> Tally {
        Tally {
            dt,
            room: vec![0.0; bins],
            receivers: vec![vec![0.0; bins]; receivers],
            paths: (0.0, 0.0, 0.0),
        }
    }

    /// Adds `value` spread evenly over `[t0, t1)` into `bins`.
    fn spread(bins: &mut [f64], dt: f64, t0: f64, t1: f64, value: f64) {
        if t1 <= t0 {
            return;
        }
        let rate = value / (t1 - t0);
        // The bin index advances by itself: a time at a bin edge, divided by dt, can round down to
        // the bin before.
        let (mut t, mut k) = (t0, (t0 / dt) as usize);
        while t < t1 && k < bins.len() {
            let edge = ((k + 1) as f64 * dt).min(t1);
            if edge > t {
                bins[k] += rate * (edge - t);
                t = edge;
            }
            k += 1;
        }
    }

    fn add(&mut self, o: &Tally) {
        for (a, b) in self.room.iter_mut().zip(&o.room) {
            *a += b;
        }
        for (r, s) in self.receivers.iter_mut().zip(&o.receivers) {
            for (a, b) in r.iter_mut().zip(s) {
                *a += b;
            }
        }
        self.paths.0 += o.paths.0;
        self.paths.1 += o.paths.1;
        self.paths.2 += o.paths.2;
    }
}

/// `rays` rays in `room` of absorption `alpha` for `duration` s, tallied at steps `dts`.
fn trace(room: &Room, alpha: f64, rays: u64, duration: f64, dts: &[f64], seed: u64) -> Vec<Tally> {
    let mut rng = Rng(seed);
    let bins = |dt: f64| (duration / dt).ceil() as usize;
    let mut tallies: Vec<Tally> = dts.iter().map(|&dt| Tally::new(dt, bins(dt), 3)).collect();
    let l = room.size;
    for _ in 0..rays {
        // Isotropic from the source.
        let z = 2.0 * rng.uniform() - 1.0;
        let phi = 2.0 * PI * rng.uniform();
        let s = (1.0 - z * z).sqrt();
        let mut d = [s * phi.cos(), s * phi.sin(), z];
        let mut p = room.source;
        let (mut t, mut w) = (0.0f64, 1.0f64);
        let mut reflected = false;
        while t < duration {
            // The nearest wall along d.
            let (mut run, mut axis) = (f64::INFINITY, 0);
            for i in 0..3 {
                if d[i] != 0.0 {
                    let bound = if d[i] > 0.0 { l[i] } else { 0.0 };
                    let x = (bound - p[i]) / d[i];
                    if x < run {
                        run = x;
                        axis = i;
                    }
                }
            }
            let run = run.max(0.0);
            let t1 = t + run / C;
            // The room's energy at the first step, and each receiver's: w times the path inside the
            // ball.
            let first = &mut tallies[0];
            Tally::spread(&mut first.room, first.dt, t, t1, w * (t1 - t));
            for (ri, q) in room.receivers.iter().enumerate() {
                let m = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
                let b = m[0] * d[0] + m[1] * d[1] + m[2] * d[2];
                let cc = m[0] * m[0] + m[1] * m[1] + m[2] * m[2] - RADIUS * RADIUS;
                let disc = b * b - cc;
                if disc <= 0.0 {
                    continue;
                }
                let r = disc.sqrt();
                let (x0, x1) = ((-b - r).max(0.0), (-b + r).min(run));
                if x1 <= x0 {
                    continue;
                }
                for tally in &mut tallies {
                    Tally::spread(
                        &mut tally.receivers[ri],
                        tally.dt,
                        t + x0 / C,
                        t + x1 / C,
                        w * (x1 - x0),
                    );
                }
            }
            if reflected {
                let tl = &mut tallies[0].paths;
                tl.0 += 1.0;
                tl.1 += run;
                tl.2 += run * run;
            }
            // To the wall, absorb, and reflect by Lambert's law about the inward normal.
            for i in 0..3 {
                p[i] += run * d[i];
            }
            p[axis] = if d[axis] > 0.0 { l[axis] } else { 0.0 };
            t = t1;
            w *= 1.0 - alpha;
            let inward = if d[axis] > 0.0 { -1.0 } else { 1.0 };
            let cos_t = rng.uniform().sqrt();
            let sin_t = (1.0 - cos_t * cos_t).sqrt();
            let psi = 2.0 * PI * rng.uniform();
            let (a1, a2) = ((axis + 1) % 3, (axis + 2) % 3);
            d[axis] = inward * cos_t;
            d[a1] = sin_t * psi.cos();
            d[a2] = sin_t * psi.sin();
            reflected = true;
        }
    }
    tallies
}

/// Least-squares slope, dB/s, of `10·lg(S)` of the backward sums of `bins` at the bin edges from
/// the first where the level is at or below `top` to the last at or above `bottom`.
fn edge_slope(bins: &[f64], dt: f64, top: f64, bottom: f64) -> f64 {
    let mut s = vec![0.0; bins.len() + 1];
    for k in (0..bins.len()).rev() {
        s[k] = s[k + 1] + bins[k];
    }
    let (mut n, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (k, v) in s.iter().enumerate() {
        let l = 10.0 * (v / s[0]).log10();
        if l <= top && l >= bottom {
            let x = k as f64 * dt;
            n += 1.0;
            sx += x;
            sy += l;
            sxx += x * x;
            sxy += x * l;
        }
    }
    (n * sxy - sx * sy) / (n * sxx - sx * sx)
}

/// The decay rate, 1/s, a renewal process of free paths `ℓ` with absorption `α` settles to: `λ`
/// with `E[(1 − α)·e^{λℓ/c}] = 1`, the paths drawn again by the same transport.
fn renewal_rate(room: &Room, alpha: f64, seed: u64) -> f64 {
    // Free paths of a long run, from the second reflection on.
    let mut rng = Rng(seed);
    let l = room.size;
    let mut p = room.source;
    let mut d = [0.6, 0.64, 0.48];
    let mut paths = Vec::with_capacity(2_000_000);
    for n in 0..2_000_001 {
        let (mut run, mut axis) = (f64::INFINITY, 0);
        for i in 0..3 {
            if d[i] != 0.0 {
                let bound = if d[i] > 0.0 { l[i] } else { 0.0 };
                let x = (bound - p[i]) / d[i];
                if x < run {
                    run = x;
                    axis = i;
                }
            }
        }
        if n > 0 {
            paths.push(run.max(0.0));
        }
        for i in 0..3 {
            p[i] += run * d[i];
        }
        p[axis] = if d[axis] > 0.0 { l[axis] } else { 0.0 };
        let inward = if d[axis] > 0.0 { -1.0 } else { 1.0 };
        let cos_t = rng.uniform().sqrt();
        let sin_t = (1.0 - cos_t * cos_t).sqrt();
        let psi = 2.0 * PI * rng.uniform();
        let (a1, a2) = ((axis + 1) % 3, (axis + 2) % 3);
        d[axis] = inward * cos_t;
        d[a1] = sin_t * psi.cos();
        d[a2] = sin_t * psi.sin();
    }
    // Bisection on λ: the mean of (1 − α)·e^{λℓ/c} rises with λ.
    let g = |lambda: f64| {
        paths
            .iter()
            .map(|x| (1.0 - alpha) * (lambda * x / C).exp())
            .sum::<f64>()
            / paths.len() as f64
            - 1.0
    };
    let (mut lo, mut hi) = (0.0, 1.0);
    while g(hi) < 0.0 {
        hi *= 2.0;
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if g(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[test]
#[ignore = "evidence for M8, not a gate: a from-scratch Lambert-box Monte Carlo against Eyring; run \
            on purpose"]
fn a_lambert_box_from_scratch_against_eyring() {
    let rays: u64 = std::env::var("SIMPA_LAMBERT_RAYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4_000_000);
    let replicas = 8u64;
    for room in &ROOMS {
        for alpha in [0.05f64, 0.1, 0.2, 0.4] {
            let (v, s) = room.volume_area();
            let a = -s * (1.0 - alpha).ln();
            let t_eyring = 24.0 * LN_10 / C * v / a;
            // Long enough for 60 dB past T30's range, at least 0.4 s.
            let duration = (1.6 * t_eyring * 1.12).max(0.4);
            let dts = [0.01, 0.0002];
            // Replicas in parallel, each its own seed: their spread is the noise.
            let results: Vec<Vec<Tally>> = std::thread::scope(|scope| {
                let handles: Vec<_> = (0..replicas)
                    .map(|i| {
                        scope.spawn(move || {
                            trace(
                                room,
                                alpha,
                                rays / replicas,
                                duration,
                                &dts,
                                0x1a3b_0000_0000_0000 + 1000 * i + (alpha * 100.0) as u64,
                            )
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            let mut all: Vec<Tally> = dts
                .iter()
                .map(|&dt| Tally::new(dt, (duration / dt).ceil() as usize, 3))
                .collect();
            for r in &results {
                for (a, b) in all.iter_mut().zip(r) {
                    a.add(b);
                }
            }
            let (n, s1, s2) = all[0].paths;
            let mean = s1 / n;
            let gamma2 = (s2 / n - mean * mean) / (mean * mean);
            // The room's energy: T from −5 to −35 dB of its backward sums, apart from params.
            let room_t = -60.0 / edge_slope(&all[0].room, all[0].dt, -5.0, -35.0);
            let lambda = renewal_rate(room, alpha, 0x2b4c + (alpha * 100.0) as u64);
            let t_renewal = 60.0 / (10.0 * lambda / LN_10);
            let t_kuttruff = t_eyring / (1.0 + 0.5 * gamma2 * (1.0 - alpha).ln());
            // Receivers: T30 and EDT per replica through params, as SPPS's are read (marked) and
            // with the reverberation continued back to the arrival alone (bare); per receiver.
            let ms = |v: &[f64]| {
                let n = v.len() as f64;
                let m = v.iter().sum::<f64>() / n;
                let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
                (m, sd / n.sqrt())
            };
            let mut rows = Vec::new();
            let mut all_t30 = vec![Vec::new(); dts.len()];
            for (ri, q) in room.receivers.iter().enumerate() {
                let dist = ((q[0] - room.source[0]).powi(2)
                    + (q[1] - room.source[1]).powi(2)
                    + (q[2] - room.source[2]).powi(2))
                .sqrt();
                // Per step: (T30s, marked EDTs, bare EDTs) over the replicas.
                let per_dt: Vec<(Vec<f64>, Vec<f64>, Vec<f64>)> = dts
                    .iter()
                    .enumerate()
                    .map(|(k, dt)| {
                        let (mut t30, mut marked, mut bare) = (Vec::new(), Vec::new(), Vec::new());
                        for r in &results {
                            let v = r[k].receivers[ri].clone();
                            let arrival = Arrival::spread(dist / C, RADIUS / C);
                            let s = EnergySeries::complete(*dt, v.clone()).unwrap();
                            let p =
                                evaluate(&s.clone().with_early_reverberation_unresolved(), arrival);
                            let b = evaluate(&s, arrival);
                            if let Ok(f) = &p.t30 {
                                t30.push(f.t_s);
                            }
                            if let Ok(f) = &p.edt {
                                marked.push(f.t_s);
                            }
                            if let Ok(f) = &b.edt {
                                bare.push(f.t_s);
                            }
                        }
                        (t30, marked, bare)
                    })
                    .collect();
                for (k, x) in per_dt.iter().enumerate() {
                    all_t30[k].extend(&x.0);
                }
                // The EDT truth: the finest step, as read (its readings agree there).
                let (truth, truth_se) = ms(&per_dt[1].1);
                let (b10, b10_se) = ms(&per_dt[0].2);
                let marked10 = if per_dt[0].1.len() > 1 {
                    let (m, se) = ms(&per_dt[0].1);
                    format!(
                        "{m:.4} ± {se:.4} s ({:+.2} %) in {} of {}",
                        100.0 * (m / truth - 1.0),
                        per_dt[0].1.len(),
                        results.len()
                    )
                } else {
                    format!(
                        "refused in {} of {}",
                        results.len() - per_dt[0].1.len(),
                        results.len()
                    )
                };
                rows.push(format!(
                    "R{ri} ({dist:.2} m): EDT at 0.2 ms {truth:.4} ± {truth_se:.4} s ({} of {}); \
                     at 10 ms continued back {b10:.4} ± {b10_se:.4} s ({:+.2} %), as SPPS's is \
                     read {marked10}",
                    per_dt[1].1.len(),
                    results.len(),
                    100.0 * (b10 / truth - 1.0)
                ));
            }
            for (k, dt) in dts.iter().enumerate() {
                let (t30, t30_se) = ms(&all_t30[k]);
                rows.push(format!(
                    "T30 at dt {dt}: {t30:.4} ± {t30_se:.4} s ({:+.2} % against Eyring, {} \
                     receiver-replicas)",
                    100.0 * (t30 / t_eyring - 1.0),
                    all_t30[k].len()
                ));
            }
            println!(
                "BOX {} alpha {alpha}: {rays} rays, {:.1} s; free path {mean:.4} m against 4V/S \
                 {:.4} m, gamma^2 {gamma2:.4}; Eyring {t_eyring:.4} s; room energy T {room_t:.4} s \
                 ({:+.2} %); renewal {t_renewal:.4} s ({:+.2} %); Kuttruff with this gamma^2 \
                 {t_kuttruff:.4} s ({:+.2} %)\n   {}",
                room.name,
                duration,
                4.0 * v / s,
                100.0 * (room_t / t_eyring - 1.0),
                100.0 * (t_renewal / t_eyring - 1.0),
                100.0 * (t_kuttruff / t_eyring - 1.0),
                rows.join("\n   ")
            );
        }
    }
}
