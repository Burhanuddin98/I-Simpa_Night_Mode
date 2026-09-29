//! M8a, the T30 physics bed (`docs/investigations/2026-09-29-m8a/SPEC.md`, pre-registered;
//! `docs/decision-log.md` rows 1, 10, 11 and 17).
//!
//! SPPS's T30, random and energetic, in two Lambert boxes with and without air, over ten seeds,
//! is held to Kuttruff's corrected Eyring with `γ²` from the room's geometry (A, ±5 %, its 95 %
//! interval), to a seed spread of the cell mean of 2 % (B), and to the independent transport
//! (C, 0.5 %); TCR's Eyring time to its analytic value (D, 0.5 %); and the preconditions E1 to
//! E7 must hold. Plain Eyring, Sabine and a 20×8×4 m room are reported, never gated.
//!
//! - [`file`]: the bed file and the projects the runs are built from;
//! - [`pe`]: E1, the solvers against `solvers/manifest.json`;
//! - [`run`]: the runs, through the run manager, and reading them again;
//! - [`read`]: what is kept of each run;
//! - [`transport`]: the independent transport's T30, and the committed high-count values;
//! - [`check`]: the checks and the reported statistics; [`stats`] the statistics they use;
//! - [`report`]: `report.json`, `summary.json` and [`report::evaluate`];
//! - [`output`]: the decay files.
//!
//! `simpa bed` (the CLI) runs it; `tools/gates/m8a.ps1` is its gate.

pub mod check;
pub mod file;
pub mod output;
pub mod pe;
pub mod read;
pub mod report;
pub mod run;
pub mod stats;
pub mod transport;

/// `solvers/manifest.json`, the verified solver build, as this build was compiled with it.
pub const SOLVER_MANIFEST: &str = include_str!("../../../solvers/manifest.json");

/// The project every cell is built from: tutorial 1's box at upstream's defaults
/// (`tests/fixtures/rooms/tutorial1_box.simpa`, as `m8_evidence.rs` builds M8's cells from it).
pub const TEMPLATE: &str = include_str!("../../../tests/fixtures/rooms/tutorial1_box.simpa");

/// The executables E1 checks, by the manifest's names.
pub const SOLVER_EXES: [&str; 4] = [
    "spps.exe",
    "classicalTheory.exe",
    "tetgen.exe",
    "preprocess.exe",
];

/// The limits of section 5, and the bed's own. Constants of the code, never fields of the bed
/// file: a bed file must not be a way to tune them after a result is seen.
pub mod limits {
    /// A: the 95 % interval of T30 against Kuttruff inside ±5 % (row 1).
    pub const KUTTRUFF: f64 = 0.05;
    /// B: the seed spread of the cell mean, `(max − min)/mean`.
    pub const SEED_SPREAD: f64 = 0.02;
    /// C: T30 against the transport (row 1's revisit column, "about 0.5 %").
    pub const TRANSPORT: f64 = 0.005;
    /// D: TCR's Eyring time against its analytic value.
    pub const TCR_EYRING: f64 = 0.005;
    /// E4: the transport against the committed high-count values, s.
    pub const HIGH_TOLERANCE_S: f64 = 1e-9;
    /// E5: Kuttruff as shipped against the committed transport T30s (row 17 (7)).
    pub const KUTTRUFF_AGAINST_HIGH: f64 = 0.006;
    /// The files a bed may leave on an exFAT volume (`B:`, 128 KB clusters).
    pub const EXFAT_FILES: u64 = 20_000;
    /// Each solver run is cancelled after this long, ms; a cancelled run is not OK (E2). The
    /// longest run of the matrix measured 21 minutes alone.
    pub const RUN_TIME_LIMIT_MS: u64 = 3 * 3_600_000;
}

/// `solvers/manifest.json`'s sha256 with its line ends as git stores them (`\n`), so that a
/// checkout's line-end conversion does not change it.
pub fn solver_manifest_sha256() -> String {
    pe::sha256_hex(SOLVER_MANIFEST.replace("\r\n", "\n").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use check::{AtmosphericReads, Reads, Verdict};
    use file::{BedFile, Cell, Method, Room, TcrCell};
    use read::{RefBand, Reference, RunInfo, SppsRead, T30, TcrBand, TcrRead};
    use std::collections::BTreeMap;
    use transport::{TransportT30, Transports};

    /// A small bed: one gated room, one cell and its TCR run, three seeds.
    fn small_bed() -> BedFile {
        let mut b = BedFile::m8a();
        b.rooms.retain(|r| r.name == "5x4x3");
        b.cells = vec![Cell {
            id: "5x4x3-a0.2-energetic-air-off".into(),
            room: "5x4x3".into(),
            alpha: 0.2,
            method: Method::Energetic,
            particles_per_source: 1_500_000,
            duration_s: 0.8,
            trans_epsilon: 9.0,
            air: false,
            gated: true,
        }];
        b.tcr = vec![TcrCell {
            id: "5x4x3-a0.2-tcr-air-off".into(),
            project_of: "5x4x3-a0.2-energetic-air-off".into(),
            gated: true,
        }];
        b.seeds = vec![1, 2, 3];
        b.say_no.n5.cell = "5x4x3-a0.2-energetic-air-off".into();
        b
    }

    fn info(exe_sha: &str) -> RunInfo {
        RunInfo {
            folder: "f".into(),
            status: "OK".into(),
            reasons: vec![],
            exit_class: 0,
            solver_wall_s: Some(1.0),
            solver_cpu_s: None,
            exe: "spps.exe".into(),
            exe_sha256: exe_sha.into(),
            tetgen_sha256: Some("tet".into()),
            files: 39,
            bytes: 1000,
        }
    }

    /// Seed `seed`'s run: every receiver-band at `kuttruff·(1 + offset + a small pattern)`.
    fn spps(seed: u32, kuttruff: f64, offset: f64) -> SppsRead {
        let bands: Vec<i32> = vec![125, 250, 500, 1000, 2000, 4000];
        SppsRead {
            info: info("spps"),
            bands_hz: bands.clone(),
            particles_per_source: 1_500_000,
            computation_method: 1,
            trans_epsilon: 9.0,
            time_step_s: 0.001,
            t30: (0..3)
                .map(|r| {
                    (0..6)
                        .map(|b| {
                            let u = ((seed as usize * 7 + r * 3 + b * 5) % 11) as f64 / 10.0 - 0.5;
                            T30 {
                                t: Some(kuttruff * (1.0 + offset + 0.002 * u)),
                                mc_sd: Some(0.001),
                                source: "value".into(),
                            }
                        })
                        .collect()
                })
                .collect(),
            reference: Reference {
                not_computed: None,
                volume_m3: 60.0,
                area_m2: 94.0,
                speed_of_sound_m_s: 343.2,
                constant_s_per_m: 0.161,
                gamma2: Some(0.3524),
                gamma2_se: Some(0.0001),
                mean_free_path_m: Some(2.553),
                bands: bands
                    .iter()
                    .map(|&f| RefBand {
                        freq_hz: f,
                        air_m_per_metre: None,
                        mean_absorption: 0.2,
                        lambert_walls: true,
                        uniform_absorption: true,
                        kuttruff_s: Some(kuttruff),
                        kuttruff_mc_sd: Some(1e-5),
                        kuttruff_refused: None,
                        eyring_s: Some(kuttruff / 1.04),
                        eyring_refused: None,
                    })
                    .collect(),
            },
            curves: vec![vec![None; 6]; 3],
        }
    }

    fn transports(bed: &BedFile, t: f64) -> Transports {
        let h = transport::high("5x4x3", 0.2).unwrap();
        let mut ts = Transports::default();
        let room: &Room = bed.room("5x4x3").unwrap();
        ts.by_key.insert(
            transport::key(&room.name, 0.2, None),
            Ok(TransportT30 {
                room: room.name.clone(),
                alpha: 0.2,
                air_m_per_metre: None,
                replicas: 16,
                rays_per_replica: 1,
                duration_s: 1.0,
                seed: "0x0".into(),
                t,
                se: h.se,
                room_t: h.room_t,
                room_se: h.room_se,
                receivers: vec![[t, h.se]; 3],
                curves: vec![],
            }),
        );
        ts
    }

    fn solvers() -> Vec<pe::SolverCheck> {
        SOLVER_EXES
            .iter()
            .map(|n| pe::SolverCheck {
                name: n.to_string(),
                path: n.to_string(),
                code_sha256: Some("c".into()),
                raw_sha256: Some(
                    match *n {
                        "spps.exe" => "spps",
                        "classicalTheory.exe" => "tcr",
                        "tetgen.exe" => "tet",
                        _ => "pre",
                    }
                    .into(),
                ),
                manifest_code_sha256: Some("c".into()),
                matches: true,
                detail: None,
            })
            .collect()
    }

    /// The reads of the small bed: seeds at `offsets`, TCR at `tcr_off` from its analytic time.
    fn reads(kuttruff: f64, offsets: &[f64], tcr_off: f64) -> Reads {
        let mut seeds = BTreeMap::new();
        for (i, o) in offsets.iter().enumerate() {
            seeds.insert(i as u32 + 1, Ok(spps(i as u32 + 1, kuttruff, *o)));
        }
        let mut r = Reads::default();
        r.spps.insert("5x4x3-a0.2-energetic-air-off".into(), seeds);
        let mut ti = info("tcr");
        ti.exe = "classicalTheory.exe".into();
        r.tcr.insert(
            "5x4x3-a0.2-tcr-air-off".into(),
            Ok(TcrRead {
                info: ti,
                bands: [125, 250, 500, 1000, 2000, 4000]
                    .iter()
                    .map(|&f| TcrBand {
                        freq_hz: f,
                        sabine_s: 0.52,
                        eyring_s: 0.465 * (1.0 + tcr_off),
                        analytic_sabine_s: Some(0.52),
                        analytic_eyring_s: Some(0.465),
                        analytic_refused: None,
                        air_m_per_metre: None,
                    })
                    .collect(),
            }),
        );
        r.atmospheric = AtmosphericReads {
            not_run: Some("not given".into()),
            ..Default::default()
        };
        r
    }

    fn judge(
        bed: &BedFile,
        reads: &Reads,
        ts: &Transports,
        exploratory: &[String],
    ) -> report::Report {
        report::evaluate(&report::Inputs {
            bed,
            reads,
            transports: ts,
            exploratory,
            solvers: &solvers(),
        })
    }

    #[test]
    fn a_bed_whose_every_check_holds_passes_and_one_cell_outside_tolerance_fails_it() {
        let bed = small_bed();
        let h = transport::high("5x4x3", 0.2).unwrap();
        // Kuttruff 0.4794 s, within 0.6 % of HIGH (E5); SPPS at Kuttruff with the transport at
        // HIGH's own values (E4), which is 0.36 % above Kuttruff: C's d is −0.36 %.
        let k = 0.4794;
        let ts = transports(&bed, h.t);
        let good = reads(k, &[0.0, 0.0005, -0.0005], 0.001);
        let r = judge(&bed, &good, &ts, &[]);
        assert!(r.pass, "{:#?}", r.failures);
        assert_eq!(r.cells[0].verdict, Verdict::Pass);
        assert_eq!(r.tcr[0].verdict, Verdict::Pass);
        assert!(r.failures.is_empty());

        // The cell 6 % above Kuttruff in every seed: A fails in every band, C fails, and the
        // report does not pass, naming them.
        let off = reads(k, &[0.06, 0.0605, 0.0595], 0.001);
        let r = judge(&bed, &off, &ts, &[]);
        assert!(!r.pass);
        assert_eq!(r.cells[0].verdict, Verdict::Fail);
        assert_eq!(r.cells[0].a.as_ref().unwrap().verdict, Verdict::Fail);
        assert_eq!(r.cells[0].c.as_ref().unwrap().verdict, Verdict::Fail);
        assert!(
            r.failures.iter().any(|f| f.contains(": A 125 Hz Fail")),
            "{:#?}",
            r.failures
        );

        // One seed 3 % off: B's seed spread fails, alone.
        let spread = reads(k, &[0.0, 0.0, 0.03], 0.001);
        let r = judge(&bed, &spread, &ts, &[]);
        assert!(!r.pass);
        assert_eq!(r.cells[0].b.as_ref().unwrap().verdict, Verdict::Fail);

        // TCR 1 % from its analytic time: D fails.
        let r = judge(&bed, &reads(k, &[0.0, 0.0005, -0.0005], 0.01), &ts, &[]);
        assert!(!r.pass);
        assert_eq!(r.tcr[0].verdict, Verdict::Fail);

        // A seed refused for more than its noise: the cell is not judged (E6).
        let mut refused = good.clone();
        let s = refused
            .spps
            .get_mut("5x4x3-a0.2-energetic-air-off")
            .unwrap();
        s.get_mut(&2).unwrap().as_mut().unwrap().t30[1][3] = T30 {
            t: None,
            mc_sd: None,
            source: "truncated".into(),
        };
        let r = judge(&bed, &refused, &ts, &[]);
        assert!(!r.pass && !r.preconditions.e6.holds);
        assert_eq!(r.cells[0].verdict, Verdict::NotJudged);

        // Kuttruff not bitwise equal over the seeds: E3.
        let mut unequal = good.clone();
        let s = unequal
            .spps
            .get_mut("5x4x3-a0.2-energetic-air-off")
            .unwrap();
        s.get_mut(&3).unwrap().as_mut().unwrap().reference.bands[0].kuttruff_s = Some(k + 1e-12);
        assert!(!judge(&bed, &unequal, &ts, &[]).preconditions.e3.holds);

        // A run of another spps.exe: E1.
        let mut other = good.clone();
        let s = other.spps.get_mut("5x4x3-a0.2-energetic-air-off").unwrap();
        s.get_mut(&1).unwrap().as_mut().unwrap().info.exe_sha256 = "other".into();
        let r = judge(&bed, &other, &ts, &[]);
        assert!(!r.pass && !r.preconditions.e1.holds);

        // A transport that is not the committed one: E4.
        let r = judge(&bed, &good, &transports(&bed, h.t + 1e-6), &[]);
        assert!(!r.pass && !r.preconditions.e4.holds);

        // A missing run: E2.
        let mut missing = good.clone();
        missing
            .spps
            .get_mut("5x4x3-a0.2-energetic-air-off")
            .unwrap()
            .remove(&2);
        let r = judge(&bed, &missing, &ts, &[]);
        assert!(!r.pass && !r.preconditions.e2.holds);

        // An exploratory bed never passes, whatever its numbers.
        let r = judge(&bed, &good, &ts, &["not the matrix".into()]);
        assert!(!r.pass && r.exploratory && !r.preconditions.e7.holds);

        // Says no, through the code: plain Eyring as A's reference (N3) fails a cell 4 % above
        // it; the air-off fault (N7) changes nothing with the air off.
        #[cfg(feature = "fault-injection")]
        {
            let r = crate::faults::with(crate::faults::Fault::BedEyringReference, || {
                judge(&bed, &good, &ts, &[])
            });
            assert_eq!(r.cells[0].a.as_ref().unwrap().reference, "eyring_s");
            assert!(
                r.cells[0].a.as_ref().unwrap().bands[0]
                    .interval
                    .unwrap()
                    .mean
                    > 0.035
            );
        }
    }

    #[test]
    fn the_report_validates_against_its_schema() {
        let bed = small_bed();
        let h = transport::high("5x4x3", 0.2).unwrap();
        let ts = transports(&bed, h.t);
        let mut rd = reads(0.4794, &[0.0, 0.0005, -0.0005], 0.001);
        rd.n5 = Some(Err("no run".into()));
        let schema = serde_json::to_value(report::schema()).unwrap();
        let mut schemas = boon::Schemas::new();
        let mut compiler = boon::Compiler::new();
        compiler.add_resource("bed.json", schema).unwrap();
        let idx = compiler.compile("bed.json", &mut schemas).unwrap();
        for r in [
            judge(&bed, &rd, &ts, &[]),
            judge(
                &bed,
                &Reads::default(),
                &Transports::default(),
                &["x".into()],
            ),
        ] {
            let v = serde_json::to_value(&r).unwrap();
            if let Err(e) = schemas.validate(&v, idx) {
                panic!("{e:#}");
            }
            let s = report::summary(&r, "abc");
            assert_eq!(s.pass, r.pass);
        }
    }

    #[test]
    fn the_manifest_hash_ignores_line_ends() {
        assert_eq!(solver_manifest_sha256().len(), 64);
    }
}
