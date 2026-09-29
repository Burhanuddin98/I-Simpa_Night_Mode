//! The independent transport's T30, M8a's tight cross-check (gate C) and its self-check E4
//! (`docs/investigations/2026-09-29-m8a/SPEC.md`, section 3.2).
//!
//! `params::lambert::decay` from the cell's source into its receivers' balls, `(1 − α)` per
//! reflection, air as `e^(−m·c·t)`: nothing of SPPS. **Its T30** is what
//! `crates/simpa-core/tests/params_kuttruff.rs` defines: the T30 of the energy a receiver ball
//! collects, read through `params` from the arrival, averaged over the room's receivers in each
//! replica; the mean over the replicas and its standard error. [`t30`] is that file's
//! `transport_t30` with the settings of the bed file, so in the eight air-off gated cells it must
//! reproduce [`HIGH`] to 10⁻⁹ s (E4): the transport is deterministic, and a difference is a
//! changed transport, not noise.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::file::{Room, TransportSettings};
use crate::params::EnergySeries;
use crate::params::ParamError;
use crate::params::decay::{self, Arrival};
use crate::params::lambert::{DecaySettings, Enclosure, decay};
use crate::params::room::{RtConstant, Surface, eyring_rt};

/// One of M8's eight air-off cells at high counts: the room (0 the 6×10×3 m, 1 the 5×4×3 m),
/// `α`, the receivers' T30 and its standard error, and the room energy's and its, s.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HighCell {
    pub room: usize,
    pub alpha: f64,
    pub t: f64,
    pub se: f64,
    pub room_t: f64,
    pub room_se: f64,
}

/// The rooms of [`HighCell::room`].
pub const HIGH_ROOMS: [&str; 2] = ["6x10x3", "5x4x3"];

/// The transport's T30 in M8's eight cells at 16 times the suite's rays (4 M to 34 M a cell),
/// from `params_kuttruff.rs`' `kuttruff_against_the_transport_at_high_counts` in a release build,
/// which re-derives each and requires it equal (to 10⁻⁹): the transport and its settings are
/// deterministic. Regenerate from that test's `HighCell` lines only after a reviewed change to the
/// transport or to `params::decay`. `params_kuttruff.rs` reads them from here.
pub const HIGH: [HighCell; 8] = [
    HighCell {
        room: 0,
        alpha: 0.05,
        t: 2.647681680,
        se: 0.000411370,
        room_t: 2.647489334,
        room_se: 0.000061000,
    },
    HighCell {
        room: 0,
        alpha: 0.1,
        t: 1.304672095,
        se: 0.000242639,
        room_t: 1.304451001,
        room_se: 0.000035608,
    },
    HighCell {
        room: 0,
        alpha: 0.2,
        t: 0.631223921,
        se: 0.000109630,
        room_t: 0.631200878,
        room_se: 0.000023486,
    },
    HighCell {
        room: 0,
        alpha: 0.4,
        t: 0.290834217,
        se: 0.000068707,
        room_t: 0.290927533,
        room_se: 0.000011009,
    },
    HighCell {
        room: 1,
        alpha: 0.05,
        t: 2.025626686,
        se: 0.000196265,
        room_t: 2.025813879,
        room_se: 0.000050207,
    },
    HighCell {
        room: 1,
        alpha: 0.1,
        t: 0.996933379,
        se: 0.000121952,
        room_t: 0.996993117,
        room_se: 0.000030028,
    },
    HighCell {
        room: 1,
        alpha: 0.2,
        t: 0.481140657,
        se: 0.000059798,
        room_t: 0.481062394,
        room_se: 0.000016347,
    },
    HighCell {
        room: 1,
        alpha: 0.4,
        t: 0.219812161,
        se: 0.000026423,
        room_t: 0.219815591,
        room_se: 0.000004383,
    },
];

/// The committed high-count values of `room` at `alpha`, if it is one of the eight.
pub fn high(room: &str, alpha: f64) -> Option<&'static HighCell> {
    HIGH.iter()
        .find(|h| HIGH_ROOMS[h.room] == room && h.alpha == alpha)
}

/// The transport's T30 in one room, `α` and air.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TransportT30 {
    pub room: String,
    pub alpha: f64,
    /// `m`, 1/m; `None` with the air off.
    pub air_m_per_metre: Option<f64>,
    pub replicas: u32,
    pub rays_per_replica: u32,
    pub duration_s: f64,
    /// `0x` and 16 hex digits.
    pub seed: String,
    /// The receivers' T30: the mean over replicas of the receivers' mean, and its standard error.
    pub t: f64,
    pub se: f64,
    /// The room energy's T30 and its standard error.
    pub room_t: f64,
    pub room_se: f64,
    /// Each receiver's T30 over the replicas: its mean and standard error.
    pub receivers: Vec<[f64; 2]>,
    /// Each receiver's Schroeder curve of the energy summed over the replicas, `[u, dB]` from the
    /// arrival (`params::decay::decay_curve`), for the bed's decay files; not in the report.
    #[serde(skip)]
    #[schemars(skip)]
    pub curves: Vec<(f64, Vec<[f64; 2]>)>,
}

/// Mean and standard error of the mean, as `params_kuttruff.rs` computes them.
fn mean_se(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

/// The transport's T30 in `room` with every face at `alpha` and the air `air` (1/m), with the
/// bed's `settings`; `rays` in place of the settings' count when given (tests only use that).
/// With [`crate::faults::Fault::BedTransportAirOff`] set, the air is off whatever `air` says.
pub fn t30(
    room: &Room,
    alpha: f64,
    air: Option<f64>,
    settings: &TransportSettings,
    rays: Option<u32>,
) -> Result<TransportT30, ParamError> {
    let air = match crate::faults::active() {
        Some(crate::faults::Fault::BedTransportAirOff) => None,
        _ => air,
    };
    let c = settings.speed_of_sound_m_s;
    let radius = settings.receiver_radius_m;
    let k = RtConstant::Physical { speed_of_sound: c };
    let e = Enclosure::shoebox(room.size_m)?;
    let walls = [Surface {
        area_m2: e.area_m2(),
        absorption: alpha,
    }];
    let t_eyring = eyring_rt(e.volume_m3(), &walls, air, k)?;
    let rays = rays.unwrap_or_else(|| settings.rays_per_replica(alpha));
    let seed = settings.seed(alpha).map_err(|detail| ParamError::BadRoom {
        field: format!("transport seed: {detail}"),
        value: alpha,
    })?;
    let duration_s = settings.duration_eyring_factor * t_eyring;
    let d = decay(
        &e,
        &vec![alpha; e.face_count()],
        &DecaySettings {
            source_m: room.source_m,
            receivers_m: room.receivers_m.clone(),
            receiver_radius_m: radius,
            speed_of_sound_m_s: c,
            time_step_s: settings.time_step_s,
            duration_s,
            air_m_per_metre: air,
            replicas: settings.replicas,
            rays_per_replica: rays,
            seed,
        },
    )?;
    let arrivals: Vec<Arrival> = room
        .receivers_m
        .iter()
        .map(|q| {
            let s = room.source_m;
            let r = ((q[0] - s[0]).powi(2) + (q[1] - s[1]).powi(2) + (q[2] - s[2]).powi(2)).sqrt();
            Arrival::spread(r / c, radius / c)
        })
        .collect();
    let mut per_receiver = Vec::new();
    let mut receivers = Vec::new();
    for (i, a) in arrivals.iter().enumerate() {
        let o = d.receiver_t30(i, *a)?;
        receivers.push([o.mean, o.se]);
        per_receiver.push(o.values);
    }
    let n = room.receivers_m.len() as f64;
    let means: Vec<f64> = (0..settings.replicas as usize)
        .map(|k| per_receiver.iter().map(|v| v[k]).sum::<f64>() / n)
        .collect();
    let (t, se) = mean_se(&means);
    let room_t30 = d.room_t30()?;
    let curves = arrivals
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let bins = d.replicas[0].receivers[i].len();
            let sum: Vec<f64> = (0..bins)
                .map(|b| d.replicas.iter().map(|r| r.receivers[i][b]).sum())
                .collect();
            match EnergySeries::new(d.time_step_s, sum) {
                Ok(s) => {
                    let curve = decay::decay_curve(&s, *a);
                    (curve.from_s, curve.points)
                }
                Err(_) => (f64::NAN, Vec::new()),
            }
        })
        .collect();
    Ok(TransportT30 {
        room: room.name.clone(),
        alpha,
        air_m_per_metre: air,
        replicas: settings.replicas,
        rays_per_replica: rays,
        duration_s,
        seed: format!("0x{seed:016x}"),
        t,
        se,
        room_t: room_t30.mean,
        room_se: room_t30.se,
        receivers,
        curves,
    })
}

/// The key of a transport: room, `α` and air, exactly.
pub fn key(room: &str, alpha: f64, air: Option<f64>) -> String {
    format!(
        "{room}|{:016x}|{}",
        alpha.to_bits(),
        air.map_or("off".to_string(), |m| format!("{:016x}", m.to_bits()))
    )
}

/// Every transport the bed needs, by [`key`]; each traced once, whatever cells share it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Transports {
    pub by_key: BTreeMap<String, Result<TransportT30, String>>,
}

impl Transports {
    /// The transport of `room`, `alpha` and `air`, traced if it is not yet.
    pub fn get_or_trace(
        &mut self,
        room: &Room,
        alpha: f64,
        air: Option<f64>,
        settings: &TransportSettings,
    ) -> &Result<TransportT30, String> {
        // The fault changes what is traced, so it changes the key too.
        let traced_air = match crate::faults::active() {
            Some(crate::faults::Fault::BedTransportAirOff) => None,
            _ => air,
        };
        let k = key(&room.name, alpha, traced_air);
        self.by_key
            .entry(k)
            .or_insert_with(|| t30(room, alpha, air, settings, None).map_err(|e| e.to_string()))
    }

    /// The transport of `room`, `alpha` and `air`, if it was traced.
    pub fn get(
        &self,
        room: &str,
        alpha: f64,
        air: Option<f64>,
    ) -> Option<&Result<TransportT30, String>> {
        let air = match crate::faults::active() {
            Some(crate::faults::Fault::BedTransportAirOff) => None,
            _ => air,
        };
        self.by_key.get(&key(room, alpha, air))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bed::file::BedFile;

    #[test]
    fn a_low_count_transport_is_the_committed_high_count_one_within_its_noise() {
        // The 5×4×3 m room at α 0.4 with 1/64 of the bed's rays: the same transport, settings and
        // seed as the committed high-count values, so it must land within a few of its standard
        // errors of them; and it is deterministic.
        let bed = BedFile::m8a();
        let room = bed.room("5x4x3").unwrap();
        let rays = bed.transport.rays_per_replica(0.4) / 64;
        let a = t30(room, 0.4, None, &bed.transport, Some(rays)).unwrap();
        let b = t30(room, 0.4, None, &bed.transport, Some(rays)).unwrap();
        assert_eq!(a, b);
        let h = high("5x4x3", 0.4).unwrap();
        let off = (a.t - h.t) / a.se.hypot(h.se);
        let off_room = (a.room_t - h.room_t) / a.room_se.hypot(h.room_se);
        assert!(
            off.abs() < 4.0 && off_room.abs() < 4.0,
            "{off} {off_room} {a:?}"
        );
        assert_eq!(a.receivers.len(), 3);
        assert_eq!(a.curves.len(), 3);
        assert!(a.curves.iter().all(|(_, p)| p.len() > 2));
        // Says no: the transport reflecting evenly over the hemisphere is far from them.
        #[cfg(feature = "fault-injection")]
        {
            let bad = crate::faults::with(crate::faults::Fault::LambertUniformReflection, || {
                t30(room, 0.4, None, &bed.transport, Some(rays)).unwrap()
            });
            let off = (bad.room_t - h.room_t) / bad.room_se.hypot(h.room_se);
            assert!(off.abs() > 4.0, "{off}");
        }
    }

    /// E4 before the bed runs: at the bed's own settings, the transport is the committed
    /// high-count values in all eight cells, to 10⁻⁹ s. A few minutes in a release build.
    #[test]
    #[ignore = "traces the eight cells at the bed's rays, minutes in a release build; run on purpose                 with --release"]
    fn the_beds_transport_is_high_in_the_eight_cells() {
        let bed = BedFile::m8a();
        let mut worst: f64 = 0.0;
        for h in HIGH {
            let room = bed.room(HIGH_ROOMS[h.room]).unwrap();
            let t = t30(room, h.alpha, None, &bed.transport, None).unwrap();
            let d = [
                t.t - h.t,
                t.se - h.se,
                t.room_t - h.room_t,
                t.room_se - h.room_se,
            ]
            .iter()
            .fold(0.0f64, |m, x| m.max(x.abs()));
            println!(
                "{} α {}: t {:.9} se {:.9} room_t {:.9} room_se {:.9}; largest difference {d:.3e} s",
                room.name, h.alpha, t.t, t.se, t.room_t, t.room_se
            );
            worst = worst.max(d);
        }
        assert!(worst <= crate::bed::limits::HIGH_TOLERANCE_S, "{worst}");
    }

    #[test]
    fn transports_are_traced_once_per_room_alpha_and_air() {
        let bed = BedFile::m8a();
        let mut small = bed.transport.clone();
        small.rays_factor = 0.05;
        let room = bed.room("5x4x3").unwrap();
        let mut ts = Transports::default();
        let a = ts
            .get_or_trace(room, 0.4, Some(0.0024), &small)
            .clone()
            .unwrap();
        let b = ts
            .get_or_trace(room, 0.4, Some(0.0024), &small)
            .clone()
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(ts.by_key.len(), 1);
        assert_eq!(a.air_m_per_metre, Some(0.0024));
        // Says no, through the code: with the air-off fault the air-on key is the air-off one.
        #[cfg(feature = "fault-injection")]
        crate::faults::with(crate::faults::Fault::BedTransportAirOff, || {
            let off = ts
                .get_or_trace(room, 0.4, Some(0.0024), &small)
                .clone()
                .unwrap();
            assert_eq!(off.air_m_per_metre, None);
            assert!(off.t > a.t, "air shortens the decay: {} {}", off.t, a.t);
        });
        assert_eq!(ts.by_key.len(), 2);
    }
}
